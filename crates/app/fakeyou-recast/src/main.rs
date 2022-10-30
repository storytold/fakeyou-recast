// launch as a gui app on windows without a command prompt
#![windows_subsystem = "windows"]

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat};
use iced::{executor, Application, Element, Command, Settings, window::{self, Icon}, Subscription, keyboard::{self, KeyCode}, Color, time};
use iced_native::{event, subscription, Event};
use log::{info, debug,  warn, error};
use model_serde::serde::deserializer::ModelDeserializer;
use native_dialog::{FileDialog, MessageDialog};
use ringbuf::RingBuffer;
use rubato::{FftFixedInOut, Resampler};
use std::{path::PathBuf, env, io::Write, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex, mpsc}};
use thiserror::Error;

mod api;
mod audio;
mod consts;
mod discord;
mod softvc;
mod vad;
mod view;
mod spinner;

use audio::DeviceType;
use consts::*;
use crate::discord::spawn_discord_thread;
use softvc::Model;
use api::ApiError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("The {0} device could not be determined")]
    NoDevice(&'static str),
    #[error("No {0} file selected")]
    NoFile(&'static str),
    #[error("No CUDA-capable GPU detected. For best results, use an Nvidia GPU.\nSwitching to CPU inference.")]
    NoCuda,
    #[error("{0}")]
    StreamConfigError(#[source] cpal::DefaultStreamConfigError),
    #[error("The input stream could not be constructed.\nInput Device must support FLOAT or PCM S16LE with a buffer size of {1}")]
    BuildInputStreamError(#[source] cpal::BuildStreamError, String),
    #[error("The output stream could not be constructed.\nOutput Device must support FLOAT or PCM S16LE with a buffer size of {1}")]
    BuildOutputStreamError(#[source] cpal::BuildStreamError, String),
    #[error("The {0} stream could not be played")]
    PlayStreamError(&'static str, #[source] cpal::PlayStreamError),
    #[error("Nothing was received from the channel")]
    ChannelRecvError(#[from] std::sync::mpsc::RecvError),
    #[error("Attempted to send to a channel that had no receiver")]
    ChannelSendError/*(#[from] std::sync::mpsc::SendError)*/,
    #[error("IO Error\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("Audio format must be PCM S16LE or FLOAT32")]
    GenericFormatError,
    #[error("WAV read/write error\n{0}\nWAV files must be PCM S16LE or FLOAT32")]
    HoundError(#[from] hound::Error),
    #[error("Attempted to lock a mutex that is already locked")]
    LockError/*(#[from] std::sync::TryLockError)*/,
    #[error("Failed to load the model")]
    ModelLoadError,
    #[error("No model is loaded")]
    NoModel,
    #[error("Inference failed to produce an output")]
    ModelError(#[from] tch::TchError),
    #[error("File picker dialog failed to open for path:\n{0}")]
    FileDialogError(PathBuf),
    #[error("Resampler construction failed:\n{0}")]
    ResamplerConstructionError(#[from] rubato::ResamplerConstructionError),
    #[error("Resampling failed:\n{0}")]
    ResampleError(#[from] rubato::ResampleError)
}


const ICON: &[u8; 262144] = include_bytes!("../../../../assets/icon.data");

pub fn main() -> iced::Result {
    let _ = init_logging(); //nowhere to log an error if one happens here

    spawn_discord_thread();

    let settings = Settings {
        window: window::Settings {
            size: (WINDOW_WIDTH, WINDOW_HEIGHT),
            resizable: false,
            //decorations: false, // this removes the menubar
            icon: Some(Icon::from_rgba(ICON.to_vec(), 256, 256).unwrap()),
            ..Default::default()
        },
        default_font: Some(&view::style::BOLD_FONT),
        default_text_size: 14,
        ..Default::default()
    };
    App::run(settings)
}

fn init_logging() -> Result<(), fern::InitError> {
    std::fs::create_dir_all("./logs")?;
    let date = chrono::Local::now();
    let date_string = date.format("%Y-%m-%d");
    let file_path_str = format!("./logs/{}.txt", date_string);
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%H:%M:%S.%3f]"),
                record.level(),
                message
            ))
        })
        .level_for("fakeyou_recast", log::LevelFilter::Info)
        
        .level_for("fakeyou_recast::audio", log::LevelFilter::Info)
        //.level_for("fakeyou_recast::softvc", log::LevelFilter::Info)
        //.level_for("fakeyou_recast::vad", log::LevelFilter::Info)   // commented out because I
                                                                    // realized this leaks our
                                                                    // model architecture (as a
                                                                    // string),
                                                                    // hopefully we don't need it
                                                                    // for logging to work well
        // we only want logs from our app not from all our dependencies
        .level(log::LevelFilter::Off)
        .chain(fern::log_file(file_path_str)?)
        .apply()?;
    info!("Logging started");
    Ok(())
}

//FIXME unsafe
static mut INPUT_VOLUME: f32 = 80.0;
static mut OUTPUT_VOLUME: f32 = 80.0;

static mut VAD_CHUNK: [f32; VAD_SIZE] = [0.0f32; VAD_SIZE];

struct App {
    /// Directory the application is launched from / running in.
    current_directory: PathBuf,
    /// Utility that handles model file loading and decryption.
    model_deserializer: ModelDeserializer,
    hubert: softvc::Hubert,
    acoustic: softvc::Acoustic,
    hifigan: softvc::Hifigan,
    vad: vad::Vad,
    view_state: view::ViewState,
    input_device_list_options: Vec<String>,
    output_device_list_options: Vec<String>,
    session_token: Option<String>,
    userdata: Option<json::JsonValue>,
    record_sender: Option<mpsc::Sender<(bool, f32)>>,
    realtime_record_sender: Option<mpsc::Sender<(bool, f32, f32)>>,
    play_target_sender: Option<mpsc::Sender<(bool, f32)>>,
    play_source_sender: Option<mpsc::Sender<(bool, f32)>>,
    recording: bool,
    realtime_recording: bool,
    playing_source: bool,
    playing_target: bool,
    cpu_mode: bool,
    offline_mode: bool,
    record_vec: RecordVector,
    spinner_frames: Vec<Vec<u8>>
}

#[derive(Debug, Clone)]
/*struct RecordVector(Arc<Vec<f32>>);
impl RecordVector {
    pub fn new() -> RecordVector {
        Self(Arc::new(Vec::<f32>::new()))
    }
    pub fn set_contents(&self, inner: Vec<f32>)  {
        *self.0 = inner;
    }
    pub fn get_contents(&self) -> Vec<f32> {
        *self.0
    }
}*/
struct RecordVector(Vec<f32>);
impl RecordVector {
    pub fn new() -> RecordVector {
        Self(Vec::new())
    }
    pub fn set_contents(&mut self, inner: Vec<f32>) {
        self.0 = inner;
    }
    pub fn get_contents(&self) -> Vec<f32> {
        self.0.clone()
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    FakeYouModelBrowsePressed,
    ModelBrowsePressed,
    HifiganBrowsePressed,
    HubertBrowsePressed,
    ShowDebugPressed,
    Slider1Changed(f32),
    Slider2Changed(f32),
    RecordPressed,
    PlayTargetPressed,
    PlaySourcePressed,
    RealtimeRecordPressed,
    InputDeviceChanged(String),
    OutputDeviceChanged(String),
    InputBrowsePressed,
    OutputBrowsePressed,
    RecordingFinished(Vec<f32>),
    UsernameChanged(String),
    PasswordChanged(String),
    LoginPressed,
    LoginSuccess(String),
    LoginFailed(ApiError),
    SessionValid(json::JsonValue),
    SessionFailed(ApiError),
    SpinnerNextFrame(std::time::Instant),
    Done
}

fn handle_errors(res: Result<(), AppError>) -> Message {
    if let Some(e) = res.err() {
        error!("{}", e);
        MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text(&format!("{}", e)).show_alert().unwrap();
    }
    Message::Done
}

fn handle_recording(res: Result<Vec<f32>, AppError>) -> Message {
    if let Some(e) = res.as_ref().err() {
        error!("{}", e);
        MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text(&format!("{}", e)).show_alert().unwrap();
    } else {
        let record_vec = res.unwrap();
        return Message::RecordingFinished(record_vec);
    }
    Message::Done
}

impl Application for App {
    type Executor = executor::Default;
    type Message = crate::Message;
    type Flags = ();

    fn new(_flags: ()) -> (App, Command<Self::Message>) {
        let hubert = softvc::Hubert::new();
        let acoustic = softvc::Acoustic::new();
        let hifigan = softvc::Hifigan::new();
        let vad = vad::Vad::new();
        let mut view_state = view::ViewState::new();
        let recording = false;
        let record_sender = None;
        let realtime_record_sender = None;
        let play_target_sender = None;
        let play_source_sender = None;
        let realtime_recording = false;
        let playing_source = false;
        let playing_target = false;
        let mut input_device_list_options = audio::get_input_device_list();
        input_device_list_options.insert(0, String::from("From file"));
        let mut output_device_list_options: Vec<String> = audio::get_output_device_list();
        output_device_list_options.insert(0, String::from("To file"));
        let mut cpu_mode = false;
        let offline_mode = false;

        let spinner_frames = vec![
            spinner::FRAME1.to_vec(),
            spinner::FRAME2.to_vec(),
            spinner::FRAME3.to_vec(),
            spinner::FRAME4.to_vec(),
            spinner::FRAME5.to_vec(),
            spinner::FRAME6.to_vec(),
            spinner::FRAME7.to_vec(),
            spinner::FRAME8.to_vec(),
            spinner::FRAME9.to_vec(),
            spinner::FRAME10.to_vec(),
            spinner::FRAME11.to_vec(),
            spinner::FRAME12.to_vec(),
            spinner::FRAME13.to_vec(),
            spinner::FRAME14.to_vec(),
            spinner::FRAME15.to_vec(),
            spinner::FRAME16.to_vec(),
            spinner::FRAME17.to_vec(),
            spinner::FRAME18.to_vec(),
            spinner::FRAME19.to_vec(),
            spinner::FRAME20.to_vec(),
            spinner::FRAME21.to_vec(),
            spinner::FRAME22.to_vec(),
            spinner::FRAME23.to_vec(),
            spinner::FRAME24.to_vec(),
            spinner::FRAME25.to_vec(),
            spinner::FRAME26.to_vec(),
            spinner::FRAME27.to_vec(),
            spinner::FRAME28.to_vec(),
            spinner::FRAME29.to_vec(),
            spinner::FRAME30.to_vec(),
        ];


        let mut session_token = None;
        let userdata = None;
        if let Ok(string) = std::fs::read_to_string(".sessiontoken") {
            session_token = Some(string);
        }

        let record_vec = RecordVector::new();

        view_state.login_modal_state.show(true);

        tch::maybe_init_cuda();
        if !tch::Cuda::is_available() {
            handle_errors(Err(AppError::NoCuda));
            cpu_mode = true;
        }

        let command;
        if let Some(token) =  session_token.clone() {
           command = Command::perform(api::check_session(token), api::session_callback);
        }
        else {
            command = Command::none();
        }

        let launch_location = env::current_dir()
            .map_err(|err| error!("startup directory could not be found: {:?}", err))
            .unwrap_or(PathBuf::from(""));

        (
            App {
                current_directory: launch_location,
                model_deserializer: ModelDeserializer::new("FAKEYOU_DEVELOPMENT"), // TODO: Real secrets.
                hubert,
                acoustic,
                hifigan,
                vad,
                view_state,
                input_device_list_options,
                output_device_list_options,
                session_token,
                userdata,
                recording,
                record_sender,
                realtime_recording,
                realtime_record_sender,
                play_target_sender,
                play_source_sender,
                playing_source,
                playing_target,
                record_vec,
                cpu_mode,
                offline_mode,
                spinner_frames,
            },
            command
        )
    }

    /// Controls the title bar of the application window
    fn title(&self) -> String {
        String::from("FakeYou Recast")
    }

    fn background_color(&self) -> Color {
        Color::new(26.0, 26.0, 39.0, 1.0)
    }

    /// UI setup and state changes
    fn view(&mut self) -> Element<Message> { view::view(&mut self.view_state, &self.input_device_list_options, &self.output_device_list_options, self.recording, self.realtime_recording, self.playing_source, self.playing_target) }

    /// Respond to messages from the UI with appropriate actions
    fn update(&mut self, msg: Self::Message) -> Command<Self::Message> {
        let mut command: Command<Self::Message> = Command::none();
        match msg {
            Message::ShowDebugPressed => {
                self.view_state.show_debug = !self.view_state.show_debug;
            }
            Message::Slider1Changed(new_value) => {
                self.view_state.slider_1_value = new_value;
                //FIXME unsafe and stupid
                unsafe { INPUT_VOLUME = new_value };
                if self.realtime_recording && self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((self.realtime_recording.clone(), self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
                if self.recording && self.record_sender.is_some() {
                    self.record_sender.as_ref().unwrap().send((self.recording.clone(), self.view_state.slider_1_value.clone())).unwrap();
                }
            }
            Message::Slider2Changed(new_value) => {
                self.view_state.slider_2_value = new_value;
                //FIXME unsafe and stupid
                unsafe { OUTPUT_VOLUME  = new_value };
                if self.realtime_recording && self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((self.realtime_recording.clone(), self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
            }
            // Realtime record button => record and playback at the same time
            Message::RealtimeRecordPressed => {
                if self.realtime_recording {
                    if self.realtime_record_sender.is_some()
                    {
                        let res = self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone()));
                        if let Some(e) = res.err() {
                           error!("{}", e); 
                        }
                    }
                } else {
                    self.view_state.show_spinner = true;
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);

                    let input_device_name = self.view_state.input_device_list_selected.clone();
                    let input_file = self.view_state.input_browse_selected.clone();
                    let output_device_name = self.view_state.output_device_list_selected.clone();
                    let output_file = self.view_state.output_browse_selected.clone();
                    let input_device_res = audio::get_input_device(input_device_name, input_file);
                    let mut input_device = audio::DeviceType::None;
                    match input_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(inp) => { input_device = inp; }
                    };
                    let output_device_res = audio::get_output_device(output_device_name, output_file);
                    let mut output_device = audio::DeviceType::None;
                    match output_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(out) => { output_device = out; }
                    };

                    let hubert = self.hubert.clone();
                    let acoustic = self.acoustic.clone();
                    let hifigan = self.hifigan.clone();
                    let vad = self.vad.clone();
                    command = Command::perform(start_realtime(realtime_record_receiver, input_device, output_device, hubert, acoustic, hifigan, vad), handle_errors);
                    let res = self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone()));
                    if let Some(e) = res.err() {
                        error!("{}", e);
                    }
                }
                self.realtime_recording = !self.realtime_recording;
            }
            Message::FakeYouModelBrowsePressed => {
                match self.handle_fakeyou_model_browse_pressed() {
                    Err(e) => {
                        handle_errors(Err(AppError::FileDialogError(self.current_directory.clone())));
                    }
                    Ok(_) => {}
                }
            }
            Message::ModelBrowsePressed => {
                let location = env::current_dir();
                if location.is_err() {
                    // how does this even happen?
                    handle_errors(Err(AppError::FileDialogError(PathBuf::from(""))));
                }
                else {
                    let mut location = location.unwrap();
                    location.push("models");
                    let path = FileDialog::new()
                    .add_filter("FakeYou Recast Model", &["recast"])
                    .set_location(&location)
                    .show_open_single_file();
                    match path {
                        Ok(Some(p)) => {
                            info!("Opening model: {}", p.clone().file_name().unwrap_or_default().to_string_lossy());
                            let acoustic = self.acoustic.clone();
                            let path = p.clone();
                            // TODO use async instead of threads and handle errors
                            std::thread::spawn(move ||{
                                acoustic.load(path)}
                            );
                            self.view_state.model_browse_selected = Some(p.clone());

                            let hifigan = self.hifigan.clone();
                            std::thread::spawn(move || {
                                hifigan.load(PathBuf::from("./models/model1.recast_base"))
                            });
                            self.view_state.hifigan_browse_selected = Some(PathBuf::from("./models/model1.recast_base"));

                            let hubert = self.hubert.clone();
                            std::thread::spawn(move ||{
                                hubert.load(PathBuf::from("./models/model2.recast_base"))}
                            );
                            self.view_state.hubert_browse_selected = Some(PathBuf::from("./models/model2.recast_base"));
                            let vad = self.vad.clone();
                            std::thread::spawn(move || {
                                vad.load(PathBuf::from("./models/silero_vad.jit"))
                            });
                        },
                        Ok(None) => {},
                        Err(_e) => { handle_errors(Err(AppError::FileDialogError(location))); }
                    }
                    //FIXME some kind of progress bar while all this crap loads
                    // I need to change these functions to async instead of this
                    // random thread crap to be able to tell when they're finished
                }
            }
            Message::HifiganBrowsePressed => { 
                let location = env::current_dir();
                if location.is_err() {
                    // how does this even happen?
                    handle_errors(Err(AppError::FileDialogError(PathBuf::from(""))));
                }
                else {
                    let mut location = location.unwrap();
                    location.push("models");
                    let path = FileDialog::new()
                        // Hifigan shhhh
                        .add_filter("Base Model 1", &["pt"])
                        .set_location(&location)
                        .show_open_single_file();
                    match path {
                        Ok(Some(p)) => {
                            let hifigan = self.hifigan.clone();
                            let path = p.clone();
                            std::thread::spawn(move ||{
                                hifigan.load(path)}
                            );
                            self.view_state.hifigan_browse_selected = Some(p.clone());
                        },
                        Ok(None) => {},
                        Err(_e) => { handle_errors(Err(AppError::FileDialogError(location))); }
                    }
                }
            }
            Message::HubertBrowsePressed => { 
                let location = env::current_dir();
                if location.is_err() {
                    // how does this even happen?
                    handle_errors(Err(AppError::FileDialogError(PathBuf::from(""))));
                }
                else {
                    let mut location = location.unwrap();
                    location.push("models");
                    let path = FileDialog::new()
                        // Hubert shhhh
                        .add_filter("Debug Model", &["pt"])
                        .set_location(&location)
                        .show_open_single_file();
                    match path {
                        Ok(Some(p)) => {
                            let hubert = self.hubert.clone();
                            let path = p.clone();
                            std::thread::spawn(move ||{
                                hubert.load(path)}
                            );
                            self.view_state.hubert_browse_selected = Some(p.clone());
                        },
                        Ok(None) => {}
                        Err(_e) => { handle_errors(Err(AppError::FileDialogError(location))); }
                    }
                }
            }
            Message::InputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .show_open_single_file();
                match path {
                    Ok(i) => {
                        info!("Selected file {} for input", i.clone().unwrap_or_default().file_name().unwrap_or_default().to_string_lossy());
                        self.view_state.input_browse_selected = i;
                    },
                    Err(_e) => { handle_errors(Err(AppError::FileDialogError(PathBuf::from("")))); }
                }
            }
            Message::OutputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .set_filename("output.wav")
                    .show_save_single_file();
                match path {
                    Ok(o) => {
                        info!("Selected file {} for output", o.clone().unwrap_or_default().file_name().unwrap_or_default().to_string_lossy());
                        self.view_state.output_browse_selected = o;
                    },
                    Err(_e) => { handle_errors(Err(AppError::FileDialogError(PathBuf::from("")))); }
                }
            }
            Message::RecordPressed => { 
                let input_device_name = self.view_state.input_device_list_selected.clone();
                if self.recording && input_device_name != Some(String::from("From file")) {
                    if self.record_sender.is_some()
                    {
                        let res = self.record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone()));
                        if let Some(e) = res.err() {
                            error!("{}", e);
                        }
                        self.recording = !self.recording;
                    }
                } else {
                    self.view_state.show_spinner = true;
                    let (record_sender, record_receiver) = std::sync::mpsc::channel();
                    self.record_sender = Some(record_sender);
                    let input_file = self.view_state.input_browse_selected.clone();
                    let input_device_res = audio::get_input_device(input_device_name.clone(), input_file);
                    let mut input_device = audio::DeviceType::None;
                    match input_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(inp) => { input_device = inp; }
                    };

                    command = Command::perform(start_recording(input_device, record_receiver), handle_recording); 
                    if input_device_name != Some(String::from("From file")) && self.record_sender.is_some() {
                        let _ = self.record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone()));
                        self.recording = !self.recording;
                    }
                }


            }
            Message::PlayTargetPressed => {
                if self.playing_target {
                    if self.play_target_sender.is_some()
                    {
                        let res = self.play_target_sender.as_ref().unwrap().send((false, self.view_state.slider_2_value.clone()));
                        if let Some(e) = res.err() {
                            error!("{}", e);
                        }
                        self.playing_target = !self.playing_target;
                    }
                } else {
                    let (play_target_sender, play_target_receiver) = std::sync::mpsc::channel();
                    self.play_target_sender = Some(play_target_sender);
                    self.view_state.show_spinner = true;
                    let output_device_name = self.view_state.output_device_list_selected.clone();
                    let output_file = self.view_state.output_browse_selected.clone();
                    let hubert = self.hubert.clone();
                    let acoustic = self.acoustic.clone();
                    let hifigan = self.hifigan.clone();
                    let vad = self.vad.clone();

                        let output_device_name = self.view_state.output_device_list_selected.clone();
                    let output_file = self.view_state.output_browse_selected.clone();
                    let hubert = self.hubert.clone();
                    let acoustic = self.acoustic.clone();
                    let hifigan = self.hifigan.clone();
                    let vad = self.vad.clone();

                    let output_device_res = audio::get_output_device(output_device_name.clone(), output_file);
                    let mut output_device = audio::DeviceType::None;
                    match output_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(out) => { output_device = out; }
                    };
                    command = Command::perform(play_target(self.record_vec.get_contents(), hubert, acoustic, hifigan, vad, output_device, play_target_receiver), handle_errors);

                    if output_device_name != Some(String::from("To file")) && self.play_target_sender.is_some() {
                        self.play_target_sender.as_ref().unwrap().send((true, self.view_state.slider_2_value.clone())).unwrap();
                        self.playing_target = !self.playing_target;
                    }
                }
            }
            Message::PlaySourcePressed => { 
                if self.playing_source {
                    if self.play_source_sender.is_some()
                    {
                        let res = self.play_source_sender.as_ref().unwrap().send((false, self.view_state.slider_2_value.clone()));
                        if let Some(e) = res.err() {
                            error!("{}", e);
                        }
                        self.playing_source = !self.playing_source;
                    }
                } else {
                    let (play_source_sender, play_source_receiver) = std::sync::mpsc::channel();
                    self.play_source_sender = Some(play_source_sender);
                    self.view_state.show_spinner = true;
                    let output_device_name = self.view_state.output_device_list_selected.clone();

                    let output_device_name = self.view_state.output_device_list_selected.clone();

                    // pass None for file because playing source to file is pointless and confusing
                    let output_device_res = audio::get_output_device(output_device_name, None);
                    let mut output_device = audio::DeviceType::None;
                    match output_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(out) => { output_device = out; }
                    };
                    command = Command::perform(play_source(self.record_vec.get_contents(), output_device, play_source_receiver), handle_errors);

                    if self.play_source_sender.is_some() {
                        self.play_source_sender.as_ref().unwrap().send((true, self.view_state.slider_2_value.clone())).unwrap();
                        self.playing_source = !self.playing_source;
                    }
                }
            }
            Message::InputDeviceChanged(new_device) => {
                info!("Selected {} for input", new_device);
                self.view_state.input_device_list_selected = Some(new_device);
            }
            Message::OutputDeviceChanged(new_device) => {
                info!("Selected {} for output", new_device);
                self.view_state.output_device_list_selected = Some(new_device);
            }

            Message::RecordingFinished(vec) => {
                self.view_state.show_spinner = false;
                self.record_vec.set_contents(vec);
            }
            Message::UsernameChanged(username) => {
                self.view_state.login_modal_state.inner_mut().username_field_value = username;
            }
            Message::PasswordChanged(password) => {
                self.view_state.login_modal_state.inner_mut().password_field_value = password;
            }
            Message::LoginPressed => {
                let username = self.view_state.login_modal_state.inner().username_field_value.clone();
                let password = self.view_state.login_modal_state.inner().password_field_value.clone();

                command = Command::perform(api::login(username, password), api::login_callback);
            }
            Message::LoginSuccess(token) => {
                match std::fs::File::create(".sessiontoken") {
                    Ok(mut file) => { let _ = file.write_all(token.as_bytes()); },
                    Err(e) => { error!("Failed to write the session token to file: {}", e); }
                }
                self.offline_mode = false;
                self.view_state.login_modal_state.inner_mut().login_failed = false;
                self.view_state.login_modal_state.show(false);
            }
            Message::LoginFailed(e) => {
                self.view_state.login_modal_state.inner_mut().login_err_msg = format!("{}", e);
                self.view_state.login_modal_state.inner_mut().login_failed = true;
                if e == ApiError::ReqwestError {
                    self.offline_mode = true;
                }
            }
            Message::SessionValid(user) => {
                self.offline_mode = false;
                self.userdata = Some(user);
                self.view_state.login_modal_state.show(false);
            }
            Message::SessionFailed(e) => {
                error!("Error validating session: {}", e);
                if e == ApiError::ReqwestError {
                    self.offline_mode = true;
                }
            }
            Message::SpinnerNextFrame(_) => {
               self.view_state.spinner_frame_index += 1; 
               if self.view_state.spinner_frame_index == 30 {
                   self.view_state.spinner_frame_index = 0;
               }
               self.view_state.spinner_frame = self.spinner_frames[self.view_state.spinner_frame_index as usize].clone();
            }
            Message::Done => {
                self.view_state.show_spinner = false;
            }
        }
        command
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.view_state.show_spinner {
            time::every(core::time::Duration::from_millis(33)).map(Message::SpinnerNextFrame)
        }
        else {
            Subscription::none()
        }
    }


}

impl App {
    fn handle_fakeyou_model_browse_pressed(&mut self) -> Result<(), AppError> {
        let mut model_directory = self.current_directory.clone();
        model_directory.push("models");

        let path = FileDialog::new()
            .add_filter("FakeYou Model", &["fakeyou"])
            .set_location(&model_directory)
            .show_open_single_file();

        match path {
            Ok(Some(p)) => {
                let model = self.model_deserializer.read_full_model_file(&p)
                    .map_err(|_e| AppError::FileDialogError(model_directory.clone()))?;

                let acoustic = self.acoustic.clone();
                let hifigan = self.hifigan.clone();

                // TODO use async instead of threads and handle errors
                std::thread::spawn(move ||{
                    acoustic.load_bytes(&model.acoustic_model_bytes)
                        .map_err(|e| {
                            e
                        })
                        .unwrap();
                    hifigan.load_bytes(&model.vocoder_model_bytes)
                        .map_err(|e| {
                            e
                        })
                        .unwrap();
                });

                self.view_state.fakeyou_model_browse_selected = Some(p.clone());

                let hubert = self.hubert.clone();
                std::thread::spawn(move ||{
                    hubert.load(PathBuf::from("./models/model2.recast_base"))}
                );

                self.view_state.hubert_browse_selected = Some(PathBuf::from("./models/model2.recast_base"));

                let vad = self.vad.clone();
                std::thread::spawn(move || {
                    vad.load(PathBuf::from("./models/silero_vad.jit"))
                });
            },
            Ok(None) => {},
            Err(_e) => { handle_errors(Err(AppError::FileDialogError(model_directory))); }
        }
        //FIXME some kind of progress bar while all this crap loads
        Ok(())
    }
}

//FIXME break this up and move parts of it into audio.rs
/// Realtime record => record and playback at the same time
async fn start_realtime(realtime_receiver: std::sync::mpsc::Receiver<(bool, f32, f32)>, input_device: audio::DeviceType, output_device: audio::DeviceType, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, vad: vad::Vad) -> Result<(), AppError>
{
    info!("Started realtime recording from {} with playback on {}", input_device, output_device);
    let arc_cancel_token = std::sync::Arc::new(AtomicBool::new(false));
    let cancel_token1 = arc_cancel_token.clone();
    let cancel_token2 = arc_cancel_token.clone();
    let cancel_token3 = arc_cancel_token.clone();
    let cancel_token4 = arc_cancel_token.clone();

    let mut cpal_input_device = None;
    match input_device {
        DeviceType::File(_f) => {
            let _ = MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text("Cannot use files for realtime.").show_alert();
        },
        DeviceType::Device(cpal_dev) => {
            cpal_input_device = Some(cpal_dev);
        }
        DeviceType::None => { Err(AppError::NoDevice("input"))?; }
    }

    let mut cpal_output_device = None;
    match output_device {
        DeviceType::File(_f) => {
            let _ = MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text("Cannot use files for realtime.").show_alert();
        },
        DeviceType::Device(cpal_dev) => {
            cpal_output_device = Some(cpal_dev);
        }
        DeviceType::None => { Err(AppError::NoDevice("output"))?; }
    }

    let input_config = cpal_input_device.as_ref().ok_or(AppError::NoDevice("input"))?.default_input_config().map_err(|_e| AppError::NoDevice("input"))?;
    let output_config = cpal_output_device.as_ref().ok_or(AppError::NoDevice("output"))?.default_output_config().map_err(|_e| AppError::NoDevice("output"))?;

    let in_rate = input_config.sample_rate().0 as usize;
    let in_n_channels = input_config.channels() as usize;
    let in_format = input_config.sample_format();
    
    let out_rate = output_config.sample_rate().0 as usize;
    let out_n_channels = output_config.channels() as usize;
    let out_format = output_config.sample_format();

    let in_chunk_size = (0.4 * in_rate as f32 * in_n_channels as f32) as usize; // 400ms
    let out_chunk_size = (0.4 * out_rate as f32 * out_n_channels as f32) as usize; // 400ms
                                                                                   
    info!("Input configured as: {} channels at {} Hz with format {:?}", in_n_channels, in_rate, in_format);
    info!("Output configured as: {} channels at {} Hz with format {:?}", out_n_channels, out_rate, out_format);

    let mut resampler1 = FftFixedInOut::<f32>::new(in_rate, MODEL_SAMPLE_RATE, (in_chunk_size as f32 / in_n_channels as f32 * MODEL_SAMPLE_RATE as f32 / in_rate as f32) as usize, 1).map_err(|e| AppError::ResamplerConstructionError(e))?;
    let mut resampler1_input_buffer = resampler1.input_buffer_allocate();
    let mut resampler1_output_buffer = resampler1.output_buffer_allocate();
    let resampler1_input_size = resampler1.input_frames_next();
    let resampler1_output_size = resampler1.output_frames_next();
    debug!("in_n_channels: {}", in_n_channels);
    debug!("in_rate: {}", in_rate);
    debug!("resampler1_input_size: {}", resampler1_input_size);
    debug!("resampler1_output_size: {}", resampler1_output_size);
    resampler1_input_buffer[0].resize(resampler1_input_size, 0.0);

    let mut resampler2 = FftFixedInOut::<f32>::new(MODEL_SAMPLE_RATE, out_rate, in_chunk_size / in_n_channels, 1).map_err(|e| AppError::ResamplerConstructionError(e))?;
    let mut resampler2_input_buffer = resampler2.input_buffer_allocate();
    let mut resampler2_output_buffer = resampler2.output_buffer_allocate();
    let resampler2_input_size = resampler2.input_frames_next();
    let resampler2_output_size = resampler2.output_frames_next();
    debug!("out_n_channels: {}", out_n_channels);
    debug!("out_rate: {}", out_rate);
    debug!("resampler2_input_size: {}", resampler2_input_size);
    debug!("resampler2_output_size: {}", resampler2_output_size);
    resampler2_input_buffer[0].resize(resampler2_input_size, 0.0);

    let ring1: RingBuffer<f32> = RingBuffer::new(in_chunk_size*2);
    let (mut producer1, mut consumer1) = ring1.split();
    let ring2: RingBuffer<f32> = RingBuffer::new(out_chunk_size*2);
    let (mut producer2, mut consumer2) = ring2.split();

    let input_data_fn = move |data: &cpal::Data, _: &cpal::InputCallbackInfo| {
        match in_format {
            SampleFormat::F32 => {
                let data_slice: &[f32] = data.as_slice().unwrap();
                for sample in data_slice.iter() {
                    let _ = producer1.push(audio::apply_volume(*sample, unsafe { INPUT_VOLUME}));
                }
            }
            SampleFormat::I16 => {
                let data_slice: &[i16] = data.as_slice().unwrap();
                for sample in data_slice.iter() {
                    let _ = producer1.push(audio::apply_volume(audio::pcm_s16_to_float(*sample), unsafe { INPUT_VOLUME}));
                }
            }
            SampleFormat::U16 => {}
        }
    };

    let output_data_fn = move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
        match out_format {
            SampleFormat::F32 => {
                let data_slice: &mut [f32] = data.as_slice_mut().unwrap();
                for sample in data_slice.iter_mut() {
                    *sample = audio::apply_volume(consumer2.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME });
                }
            }
            SampleFormat::I16 => {
                let data_slice: &mut [i16] = data.as_slice_mut().unwrap();
                for sample in data_slice.iter_mut() {
                    *sample = audio::float_to_pcm_s16(audio::apply_volume(consumer2.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME }));
                }
            }
            SampleFormat::U16 => {}
        }
    };


    std::thread::spawn(move || {
        loop {
            if consumer1.len() >= resampler1_input_size {
                let processing_start_time = std::time::Instant::now();
                // mix stereo (or more channels) to mono
                resampler1_input_buffer[0].clear();
                for _ in 0..(in_chunk_size / in_n_channels) {
                    let mut mixed: f32 = 0.0;
                    for _ in 0..in_n_channels {
                        mixed += consumer1.pop().unwrap_or(0.0); 
                    }
                    resampler1_input_buffer[0].push(mixed);
                }
                // downsample to 16k
                let res = resampler1.process_into_buffer(&resampler1_input_buffer, &mut resampler1_output_buffer, None);
                match res {
                    Ok(_) => {}
                    Err(e) => { error!("Input resampler error: {}", e) }
                };


                let mut softvc_input = resampler1_output_buffer[0].clone();
                while softvc_input.len() < TENSOR_SIZE {
                    softvc_input.extend_from_within(0..softvc_input.len());
                }
                //softvc_input.resize(TENSOR_SIZE, 0.0);

                // Find active 50ms chunks with Silero VAD
                let mut vad_confidence = vec![0.0; resampler1_output_size / VAD_SIZE+1];
                let mut v = 0;
                softvc_input[..resampler1_output_size].chunks(VAD_SIZE).for_each(|chunk| {
                    unsafe { VAD_CHUNK.fill(0.0); }
                    unsafe { VAD_CHUNK[..chunk.len()].copy_from_slice(chunk) };
                    let vad_res = vad.forward(unsafe { &VAD_CHUNK});
                    let mut vad_tensor = tch::Tensor::of_slice(&[0.0]);
                    match vad_res {
                        Err(e) => {
                            // don't spam errors if there's already one known
                            if !cancel_token1.load(Ordering::Relaxed) {
                                handle_errors(Err(e));
                            }
                            cancel_token1.store(true, Ordering::Relaxed);
                        },
                        Ok(t) => {
                            vad_tensor = t;
                        }
                    }
                    vad_confidence[v] = vad_tensor.double_value(&[0]) as f32;
                    v += 1;
                });
                
                let res = softvc::inference(&softvc_input, hubert.clone(), acoustic.clone(), hifigan.clone());
                match res {
                    Err(e) => {
                        // don't spam errors if there's already one known
                        if !cancel_token1.load(Ordering::Relaxed) {
                            handle_errors(Err(e));
                        }
                        cancel_token1.store(true, Ordering::Relaxed);
                    },
                    Ok(samples) => {
                        resampler2_input_buffer[0].iter_mut().zip(
                        samples.iter()
                            .enumerate()
                            .map(|(e, sample)| {
                                let mut sa = 0.0;
                                for i in 0..(samples.len() / VAD_SIZE)  {
                                    if i == vad_confidence.len() { break; }
                                    if e < (i+1)*VAD_SIZE && e > i*VAD_SIZE &&  vad_confidence[i] > SPEAKER_THRESHOLD {
                                        sa = *sample;
                                        break;
                                    }
                                }
                                sa
                            })
                        ).for_each(|(i2, o1)| *i2 = o1);
                        let res = resampler2.process_into_buffer(&resampler2_input_buffer, &mut resampler2_output_buffer, None);
                        match res {
                            Ok(_) => {}
                            Err(e) => { error!("Output resampler error: {}", e) }
                        };

                        resampler2_output_buffer[0].iter().for_each(|&sample| {
                            // mono to stereo etc.
                            for _ in 0..out_n_channels {
                                producer2.push(sample).unwrap_or(()); 
                            }
                        });
                    }
                }
                let processing_end_time = std::time::Instant::now();
                if cancel_token3.load(Ordering::Relaxed) {
                    break;
                }
                let processing_time = processing_end_time - processing_start_time;
                let sleep_time = resampler1_output_size as f32 / MODEL_SAMPLE_RATE as f32 - processing_time.as_secs_f32();
                if sleep_time > 0.0 {
                    std::thread::sleep(core::time::Duration::from_secs_f32(sleep_time));
                }
            }
        }
    });

    let in_dev = cpal_input_device.ok_or(AppError::NoDevice("input"))?;
    let out_dev = cpal_output_device.ok_or(AppError::NoDevice("output"))?;

    let mut input_config = input_config.config();
    input_config.buffer_size = BufferSize::Fixed(in_chunk_size as u32);
    let mut output_config = output_config.config();
    output_config.buffer_size = BufferSize::Fixed(out_chunk_size as u32);
    
    let input_stream = match in_format {
        SampleFormat::F32 => {
            in_dev.build_input_stream_raw(&input_config, SampleFormat::F32, input_data_fn, audio::err_fn).map_err(|e| AppError::BuildInputStreamError(e, format!("{}", in_chunk_size)))
        }
        SampleFormat::I16 => {
            in_dev.build_input_stream_raw(&input_config, SampleFormat::I16, input_data_fn, audio::err_fn).map_err(|e| AppError::BuildInputStreamError(e, format!("{}", in_chunk_size)))
        }
        SampleFormat::U16 => {
            Err(AppError::GenericFormatError)
        }
    }?;

    let output_stream = match out_format {
        SampleFormat::F32 => {
            out_dev.build_output_stream_raw(&output_config, SampleFormat::F32, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, format!("{}", out_chunk_size)))
        }
        SampleFormat::I16 => {
            out_dev.build_output_stream_raw(&output_config, SampleFormat::I16, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, format!("{}", out_chunk_size)))
        }
        SampleFormat::U16 => {
            Err(AppError::GenericFormatError)
        }
    }?;


    input_stream.play().map_err(|e| AppError::PlayStreamError("input", e))?;
    output_stream.play().map_err(|e| AppError::PlayStreamError("output", e))?;
    loop {
        if cancel_token2.load(Ordering::Relaxed) {
            //FIXME set the button text back to realtime instead of leaving it as stop when an
            //error is encountered.
            break;
        }

        // The receiver tells us whether the user pressed stop and the current volume
        // setting
        let (realtime_recording, ivol, ovol) = realtime_receiver.recv().map_err(|e| AppError::ChannelRecvError(e))?;
        if realtime_recording {
            //FIXME unsafe
            unsafe { INPUT_VOLUME = ivol };
            unsafe { OUTPUT_VOLUME = ovol };
        } else {
            info!("Stopped realtime recording");
            break;
        }
    }
    cancel_token4.store(true, Ordering::Relaxed);
    Ok(())
}

/// Record to a buffer for later conversion (non-realtime)
async fn start_recording(input_device: audio::DeviceType, receiver: std::sync::mpsc::Receiver<(bool, f32)>) -> Result<Vec<f32>, AppError> {
    info!("Recording started from {}", input_device);
    let vec = audio::record_audio_vec(input_device, receiver)?;
    Ok(vec)
}

/// Play back the input buffer
async fn play_source(source: Vec<f32>, output_device: audio::DeviceType, play_source_receiver: mpsc::Receiver<(bool, f32)>) -> Result<(), AppError> {
    info!("Playing source audio on {}", output_device);
    debug!("source first 20 samples {:?}", source[0..20].to_vec());
    audio::play_audio_vec(output_device, source, play_source_receiver)
}

/// Convert and play back the input buffer with the target voice
async fn play_target(source: Vec<f32>, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, vad: vad::Vad, output_device: audio::DeviceType, play_target_receiver: mpsc::Receiver<(bool, f32)>) -> Result<(), AppError> {
    //TODO some kind of loading spinner or something while this is happening
    info!("Running inference and playing target audio on {}", output_device);
    debug!("first 20 recorded samples: {:?}", source[..20].to_vec());

    match output_device {
        DeviceType::File((ref f, _)) => {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: MODEL_SAMPLE_RATE as u32,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::new(f, spec).map_err(|e| AppError::HoundError(e))?;
            'outer: for chunk in source.chunks(OFFLINE_CHUNK_SIZE) {
                // Find active 50ms chunks with Silero VAD
                let mut vad_confidence: Vec<f32> = Vec::new();
                for vad_chunk in chunk.chunks(VAD_SIZE) {
                    let mut vad_tensor = tch::Tensor::of_slice(&[0.0]);
                    let mut vad_chunk = vad_chunk.to_vec();
                    vad_chunk.resize(VAD_SIZE, 0.0);
                    let vad_res = vad.forward(&vad_chunk);
                    match vad_res {
                        Err(e) => {
                            handle_errors(Err(e));
                            break 'outer;
                        },
                        Ok(t) => {
                            vad_tensor = t;
                        }
                    }
                    vad_confidence.push(vad_tensor.double_value(&[0]) as f32);
                }

                let mut softvc_input = Vec::new();
                softvc_input.extend_from_slice(chunk);

                let mut out_samples = softvc::inference(&softvc_input, hubert.clone(), acoustic.clone(), hifigan.clone())?;

                // nuke inactive chunks from the buffer
                for v in 0..chunk.len()/VAD_SIZE {
                    if vad_confidence[v] < SPEAKER_THRESHOLD {
                        out_samples[v*VAD_SIZE..(v+1)*VAD_SIZE].fill(0.0);
                    }
                }
                for sample in out_samples.iter() {
                    writer.write_sample(audio::float_to_pcm_s16(audio::apply_volume(*sample, unsafe { OUTPUT_VOLUME }))).map_err(|e| AppError::HoundError(e))?;
                }
            }
            writer.finalize().map_err(|e| AppError::HoundError(e))?;
        },
        DeviceType::Device(ref d) => {
            let playback_secs = source.len() as f32 / 16000.0;

            let output_config = d.default_output_config().map_err(|e| AppError::StreamConfigError(e))?;
            let out_rate = output_config.sample_rate().0 as usize;
            let n_channels = output_config.channels() as usize;
            let sample_format = output_config.sample_format();
            info!("Output configured as: {} channels at {} Hz with format {:?}", n_channels, out_rate, sample_format);

            let mut resampler = FftFixedInOut::<f32>::new(MODEL_SAMPLE_RATE, out_rate, OFFLINE_CHUNK_SIZE, 1).map_err(|e| AppError::ResamplerConstructionError(e))?;
            let mut resampler_input_buffer = resampler.input_buffer_allocate();
            let mut resampler_output_buffer = resampler.output_buffer_allocate();
            let resampler_input_size = resampler.input_frames_next();
            let resampler_output_size = resampler.output_frames_next();

            debug!("resampler_input_size: {}", resampler_input_size);
            debug!("resampler_output_size: {}", resampler_output_size);

            resampler_input_buffer[0].resize(resampler_input_size, 0.0);

            let output_chunk_size = out_rate * resampler_input_size / MODEL_SAMPLE_RATE * n_channels;
            let ring: RingBuffer<f32> = RingBuffer::new(output_chunk_size*2);
            let (mut producer, mut consumer) = ring.split();

            let arc_cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel_token1 = arc_cancel_token.clone();
            let cancel_token2 = arc_cancel_token.clone();
            let cancel_token3 = arc_cancel_token.clone();

            std::thread::spawn(move || {
                for chunk in source.chunks(resampler_input_size)
                {
                    let processing_start_time = std::time::Instant::now();
                    // Find active 50ms chunks with Silero VAD
                    let mut vad_confidence: Vec<f32> = Vec::new();
                    chunk.chunks(VAD_SIZE).for_each(|vad_chunk| {
                        let mut vad_tensor = tch::Tensor::of_slice(&[0.0]);
                        let mut vad_chunk = vad_chunk.to_vec();
                        vad_chunk.resize(VAD_SIZE, 0.0);
                        let vad_res = vad.forward(&vad_chunk);
                        match vad_res {
                            Err(e) => {
                                // don't spam errors if there's already one known
                                if !cancel_token1.load(std::sync::atomic::Ordering::Relaxed) {
                                    handle_errors(Err(e));
                                }
                                cancel_token1.store(true, std::sync::atomic::Ordering::Relaxed);
                            },
                            Ok(t) => {
                                vad_tensor = t;
                            }
                        }
                        vad_confidence.push(vad_tensor.double_value(&[0]) as f32)
                    });

                    debug!("vad_confidence: {:?}", vad_confidence);

                    let softvc_out = softvc::inference(&chunk, hubert.clone(), acoustic.clone(), hifigan.clone());
                    match softvc_out {
                        Err(e) => {
                            // don't spam errors if there's already one known
                            if !cancel_token1.load(std::sync::atomic::Ordering::Relaxed) {
                                handle_errors(Err(e));
                            }
                            cancel_token1.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        Ok(mut samples) => {
                            // pad for the last chunk so we don't chop off the end of the audio
                            if samples.len() < resampler_input_size {
                                samples.resize(resampler_input_size, 0.0);
                            }
                            debug!("first 20 samples from inference: {:?}", samples[..20].to_vec()); 
                            resampler_input_buffer[0].iter_mut().zip(
                            samples.iter()
                            .enumerate()
                            .map(|(e, sample)| {
                                let mut sa = 0.0;
                                for i in 0..resampler_input_size/VAD_SIZE {
                                    if i == vad_confidence.len() { break; }
                                    if e < (i+1)*VAD_SIZE && e > i*VAD_SIZE && vad_confidence[i] > SPEAKER_THRESHOLD {
                                        sa = *sample;
                                        break;
                                    }
                                }
                                sa
                            }))
                            .for_each(|(i, o)| *i = o) ;
                        let resampler_res = resampler.process_into_buffer(&resampler_input_buffer, &mut resampler_output_buffer, None);
                        if let Err(e) = resampler_res {
                            error!("Resampler error: {}", e)
                        }
                        resampler_output_buffer[0].iter().for_each(|&sample| {
                            // mono to stereo etc.
                            for _ in 0..n_channels {
                                producer.push(sample).unwrap_or(()); 
                            }
                        });
                    }
                }
                let processing_end_time = std::time::Instant::now();
                if cancel_token3.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let processing_time = processing_end_time - processing_start_time;
                let sleep_time = chunk.len() as f32 / MODEL_SAMPLE_RATE as f32 - processing_time.as_secs_f32();
                if sleep_time > 0.0 {
                    std::thread::sleep(core::time::Duration::from_secs_f32(sleep_time));
                }
            }
        });

        let output_data_fn = move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
            match data.sample_format() {
                SampleFormat::F32 => {
                    // this unwrap is safe because it only happens if the sample format doesn't
                    // match which we just checked 
                    let data_slice: &mut [f32] = data.as_slice_mut().unwrap(); 
                    for sample in data_slice.iter_mut() {
                        *sample = audio::apply_volume(consumer.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME });
                    }
                }
                SampleFormat::I16 => {
                    // this unwrap is safe because it only happens if the sample format doesn't
                    // match which we just checked 
                    let data_slice: &mut [i16] = data.as_slice_mut().unwrap(); 
                    for sample in data_slice.iter_mut() {
                        *sample = audio::float_to_pcm_s16(audio::apply_volume(consumer.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME }));
                    }                    }
                SampleFormat::U16 => {}
            }

        };


        let output_stream = match sample_format
        {
            SampleFormat::F32 => {
                Ok(d.build_output_stream_raw(&output_config.config(), SampleFormat::F32, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, String::from("Default")))?)
            }
            SampleFormat::I16 => {
                Ok(d.build_output_stream_raw(&output_config.config(), SampleFormat::I16, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, String::from("Default")))?)
            }
            SampleFormat::U16 => {
                Err(AppError::GenericFormatError)
            }

        }?;


        output_stream.play().map_err(|e| {error!("Output stream error: {}", e); AppError::PlayStreamError("output", e)})?;
        //add a couple seconds in case we fall behind
        let stop_time = std::time::Instant::now() + core::time::Duration::from_secs_f32(playback_secs + 2.0);
        loop {
            if cancel_token2.load(std::sync::atomic::Ordering::Relaxed) || std::time::Instant::now() > stop_time {
                break;
            }

            // The receiver tells us whether the user pressed stop and the current volume
            // setting
            let (playing_target, ovol) = play_target_receiver.recv().map_err(|e| AppError::ChannelRecvError(e))?;
            if playing_target {
                //FIXME unsafe
                unsafe { OUTPUT_VOLUME = ovol };
            } else {
                break;
            }

            std::thread::sleep(core::time::Duration::from_millis(50));
        }
        cancel_token2.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    DeviceType::None => {
        Err(AppError::NoDevice("output"))?
    }
    }
    info!("Finished playing target audio");
    Ok(())
}

