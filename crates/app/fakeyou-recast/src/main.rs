// launch as a gui app on windows without a command prompt
#![windows_subsystem = "windows"]

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleRate, BufferSize, SampleFormat};
use iced::{executor, Application, Element, Command, Settings, window::{self, Icon}, Subscription, keyboard::{self, KeyCode}};
use iced_native::{event, subscription, Event};
use log::{info, debug, trace, warn, error};
use model_serialization::model_serializer::ModelSerializer;
use native_dialog::{FileDialog, MessageDialog};
use ringbuf::RingBuffer;
use rubato::{FftFixedInOut, Resampler};
use std::{path::PathBuf, env, io::Write};
use thiserror::Error;

mod api;
mod audio;
mod consts;
mod discord;
mod softvc;
mod vad;
mod view;

use audio::DeviceType;
use consts::*;
use crate::discord::spawn_discord_thread;
use softvc::Model;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("The {0} device could not be determined")]
    NoDevice(&'static str),
    #[error("No {0} file selected")]
    NoFile(&'static str),
    #[error("The stream could not be constructed.\nInput Device must support mono audio at 48kHz sample rate")]
    BuildInputStreamError(#[source] cpal::BuildStreamError),
    #[error("The stream could not be constructed.\nOutput Device must support stereo audio at 48kHz sample rate")]
    BuildOutputStreamError(#[source] cpal::BuildStreamError),
    #[error("The {0} stream could not be played")]
    PlayStreamError(&'static str, #[source] cpal::PlayStreamError),
    #[error("Nothing was received from the channel")]
    ChannelRecvError(#[from] std::sync::mpsc::RecvError),
    #[error("Attempted to send to a channel that had no receiver")]
    ChannelSendError/*(#[from] std::sync::mpsc::SendError)*/,
    #[error("IO Error\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("WAV read/write error\n{0}\nInput wav files must be 16KHz mono in 32-bit float format")]
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
            resizable: true,
            decorations: true,
            icon: Some(Icon::from_rgba(ICON.to_vec(), 256, 256).unwrap()),
            ..Default::default()
        },
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
    model_serializer: ModelSerializer,
    hubert: softvc::Hubert,
    acoustic: softvc::Acoustic,
    hifigan: softvc::Hifigan,
    vad: vad::Vad,
    view_state: view::ViewState,
    input_device_list_options: Vec<String>,
    output_device_list_options: Vec<String>,
    session_token: Option<String>,
    userdata: Option<json::JsonValue>,
    record_sender: Option<std::sync::mpsc::Sender<(bool, f32)>>,
    realtime_record_sender: Option<std::sync::mpsc::Sender<(bool, f32, f32)>>,
    recording: bool,
    realtime_recording: bool,
    record_vec: Vec<f32>,
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
    LoginFailed(api::ApiError),
    SessionValid(json::JsonValue),
    SessionFailed(api::ApiError),
    None
}

fn handle_errors(res: Result<(), AppError>) -> Message {
    if let Some(e) = res.err() {
        error!("{}", e);
        MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text(&format!("{}", e)).show_alert().unwrap();
    }
    Message::None
}

fn handle_recording(res: Result<Vec<f32>, AppError>) -> Message {
    if let Some(e) = res.as_ref().err() {
        error!("{}", e);
        MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text(&format!("{}", e)).show_alert().unwrap();
    } else {
        let record_vec = res.unwrap();
        return Message::RecordingFinished(record_vec);
    }
    Message::None
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
        let realtime_recording = false;
        let mut input_device_list_options = audio::get_input_device_list();
        input_device_list_options.insert(0, String::from("From file"));
        let mut output_device_list_options: Vec<String> = audio::get_output_device_list();
        output_device_list_options.insert(0, String::from("To file"));


        let mut session_token = None;
        let userdata = None;
        if let Ok(string) = std::fs::read_to_string(".sessiontoken") {
            session_token = Some(string);
        }

        let record_vec = Vec::new();

        view_state.login_modal_state.show(true);

        tch::maybe_init_cuda();

        let command;
        if let Some(token) =  session_token.clone() {
           command = Command::perform(api::check_session(token), api::session_callback);
        }
        else {
            command = Command::none();
        }

        let launch_location = env::current_dir()
            .map_err(|err| println!("startup directory could not be found: {:?}", err))
            .unwrap_or(PathBuf::from(""));

        (
            App {
                current_directory: launch_location,
                model_serializer: ModelSerializer::new("FAKEYOU_DEVELOPMENT"), // TODO: Real secrets.
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
                record_vec,
            },
            command
        )
    }

    /// Controls the title bar of the application window
    fn title(&self) -> String {
        String::from("FakeYou Recast")
    }

    /// UI setup and state changes
    fn view(&mut self) -> Element<Message> { view::view(&mut self.view_state, &self.input_device_list_options, &self.output_device_list_options, self.recording, self.realtime_recording) }

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
                        //handle_errors(res); 
                        //This error is kind of pointless because it's usually
                        //the result of another error, ignoring it to avoid spam
                        self.recording = !self.recording;
                    }
                } else {
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
                    if input_device_name != Some(String::from("From file")) {
                        self.record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone())).unwrap();
                        self.recording = !self.recording;
                    }
                }


            }
            Message::PlayTargetPressed => {
                let output_device_name = self.view_state.output_device_list_selected.clone();
                let output_file = self.view_state.output_browse_selected.clone();
                let hubert = self.hubert.clone();
                let acoustic = self.acoustic.clone();
                let hifigan = self.hifigan.clone();
                let vad = self.vad.clone();

                let output_device_res = audio::get_output_device(output_device_name, output_file);
                let mut output_device = audio::DeviceType::None;
                match output_device_res {
                    Err(e) => { handle_errors(Err(e)); },
                    Ok(out) => { output_device = out; }
                };
                //FIXME kind of a big clone
                command = Command::perform(play_target(self.record_vec.clone(), hubert, acoustic, hifigan, vad, output_device), handle_errors)
            }
            Message::PlaySourcePressed => { 
                let output_device_name = self.view_state.output_device_list_selected.clone();

                // pass None for file because playing source to file is pointless and confusing
                let output_device_res = audio::get_output_device(output_device_name, None);
                let mut output_device = audio::DeviceType::None;
                match output_device_res {
                    Err(e) => { handle_errors(Err(e)); },
                    Ok(out) => { output_device = out; }
                };
                //FIXME kind of a big clone
                command = Command::perform(play_source(self.record_vec.clone(), output_device), handle_errors)
            }
            Message::InputDeviceChanged(new_device) => {
                info!("Selected {} for input", new_device);
                self.view_state.input_device_list_selected = Some(new_device);
                // Restart recording if currently recording and the device changes
                //if self.realtime_recording {
                    //self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                    //let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    //self.realtime_record_sender = Some(realtime_record_sender);
                    //let input_device_name = self.view_state.input_device_list_selected.clone();
                    //let output_device_name = self.view_state.output_device_list_selected.clone();
                    //std::thread::spawn(move || { start_realtime(realtime_record_receiver, input_device_name, output_device_name) }); 
                    //self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                //}
            }
            Message::OutputDeviceChanged(new_device) => {
                info!("Selected {} for output", new_device);
                self.view_state.output_device_list_selected = Some(new_device);
                // Restart recording if currently recording and the device changes
                //if self.realtime_recording {
                    //self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                    //let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    //self.realtime_record_sender = Some(realtime_record_sender);
                    //let input_device_name = self.view_state.input_device_list_selected.clone();
                    //let output_device_name = self.view_state.output_device_list_selected.clone();
                    //std::thread::spawn(move || { start_realtime(realtime_record_receiver, input_device_name, output_device_name) }); 
                    //self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                //}

            }

            Message::RecordingFinished(vec) => {
                self.record_vec = vec;
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
                self.view_state.login_modal_state.inner_mut().login_failed = false;
                self.view_state.login_modal_state.show(false);
            }
            Message::LoginFailed(e) => {
                self.view_state.login_modal_state.inner_mut().login_err_msg = format!("{}", e);
                self.view_state.login_modal_state.inner_mut().login_failed = true;
            }
            Message::SessionValid(user) => {
                self.userdata = Some(user);
                self.view_state.login_modal_state.show(false);
            }
            Message::SessionFailed(e) => {
                error!("Error validating session: {}", e);
            }
            Message::None => { }
        }
        command
    }

    /// F12 hotkey for secret options
    fn subscription(&self) -> Subscription<Message> {
        subscription::events_with(|event, status| {
            if let event::Status::Captured = status {
                return None;
            }

            match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    modifiers: _,
                    key_code,
                }) => match key_code {
                    KeyCode::F12 => Some(Message::ShowDebugPressed),
                    _ => None,
                }
                _ => None,
            }
        })
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
                let model = self.model_serializer.read_full_model_file(&p)
                    .map_err(|e| AppError::FileDialogError(model_directory.clone()))?;

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
            Err(_e) => { /*handle_errors(Err(AppError::FileDialogError(location)));*/ }
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
    let arc_cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancel_token1 = arc_cancel_token.clone();
    let cancel_token2 = arc_cancel_token.clone();
    let cancel_token3 = arc_cancel_token.clone();
    let cancel_token4 = arc_cancel_token.clone();

    let mut cpal_input_device = None;
    match input_device {
        DeviceType::File(_f) => {
            MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text("Cannot use files for realtime.").show_alert().unwrap();
        },
        DeviceType::Device(cpal_dev) => {
            cpal_input_device = Some(cpal_dev);
        }
        DeviceType::None => { Err(AppError::NoDevice("input"))?; }
    }

    let mut cpal_output_device = None;
    match output_device {
        DeviceType::File(_f) => {
            MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text("Cannot use files for realtime.").show_alert().unwrap();
        },
        DeviceType::Device(cpal_dev) => {
            cpal_output_device = Some(cpal_dev);
        }
        DeviceType::None => { Err(AppError::NoDevice("output"))?; }
    }

    let mut input_config = cpal_input_device.as_ref().ok_or(AppError::NoDevice("input"))?.default_input_config().map_err(|_e| AppError::NoDevice("input"))?.config();
    let mut output_config = cpal_output_device.as_ref().ok_or(AppError::NoDevice("output"))?.default_output_config().map_err(|_e| AppError::NoDevice("output"))?.config();


    let in_rate = input_config.sample_rate.0 as usize;
    let in_n_channels = input_config.channels as usize;
    
    let out_rate = output_config.sample_rate.0 as usize;
    let out_n_channels = output_config.channels as usize;

    let in_chunk_size = (0.4 * in_rate as f32 * in_n_channels as f32) as usize; // 400ms
    let out_chunk_size = (0.4 * out_rate as f32 * out_n_channels as f32) as usize; // 400ms


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

    input_config.buffer_size = BufferSize::Fixed(in_chunk_size as u32);
    output_config.buffer_size = BufferSize::Fixed(out_chunk_size as u32);

    let input_data_fn = move |data: &cpal::Data, _: &cpal::InputCallbackInfo| {
        let data_slice: &[f32] = data.as_slice().unwrap();
        for sample in data_slice.iter() {
            let _ = producer1.push(audio::apply_volume(*sample, unsafe { INPUT_VOLUME}));
        }
    };

    let output_data_fn = move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
        let data_slice: &mut [f32] = data.as_slice_mut().unwrap();
        for sample in data_slice.iter_mut() {
            *sample = audio::apply_volume(consumer2.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME });
        }
    };


    std::thread::spawn(move || {
        loop {
            if consumer1.len() >= resampler1_input_size {
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
                resampler1.process_into_buffer(&resampler1_input_buffer, &mut resampler1_output_buffer, None).unwrap();


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
                            if !cancel_token1.load(std::sync::atomic::Ordering::Relaxed) {
                                handle_errors(Err(e));
                            }
                            cancel_token1.store(true, std::sync::atomic::Ordering::Relaxed);
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
                        if !cancel_token1.load(std::sync::atomic::Ordering::Relaxed) {
                            handle_errors(Err(e));
                        }
                        cancel_token1.store(true, std::sync::atomic::Ordering::Relaxed);
                    },
                    Ok(samples) => {
                        resampler2_input_buffer[0].iter_mut().zip(
                        samples.iter()
                            .enumerate()
                            .map(|(e, sample)| {
                                let mut sa = 0.0;
                                for i in 0..(samples.len() / VAD_SIZE)  {
                                    if e < (i+1)*VAD_SIZE && e > i*VAD_SIZE &&  vad_confidence[i] > SPEAKER_THRESHOLD {
                                        sa = *sample;
                                        break;
                                    }
                                }
                                sa
                            })
                        ).for_each(|(i2, o1)| *i2 = o1);
                        resampler2.process_into_buffer(&resampler2_input_buffer, &mut resampler2_output_buffer, None).unwrap();
                        resampler2_output_buffer[0].iter().for_each(|&sample| {
                            // mono to stereo etc.
                            for _ in 0..out_n_channels {
                                producer2.push(sample).unwrap_or(()); 
                            }
                        });
                    }
                }
            }

            if cancel_token3.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
        }
    });

    let input_stream = cpal_input_device.ok_or(AppError::NoDevice("input"))?.build_input_stream_raw(&input_config, SampleFormat::F32, input_data_fn, audio::err_fn).map_err(|e| AppError::BuildInputStreamError(e))?;
    let output_stream = cpal_output_device.ok_or(AppError::NoDevice("output"))?.build_output_stream_raw(&output_config, SampleFormat::F32, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e))?;
    input_stream.play().map_err(|e| AppError::PlayStreamError("input", e))?;
    output_stream.play().map_err(|e| AppError::PlayStreamError("output", e))?;
    loop {
        if cancel_token2.load(std::sync::atomic::Ordering::Relaxed) {
            //FIXME set the button text back to realtime instead of leaving it as stop.
            core::mem::drop(input_stream);
            core::mem::drop(output_stream);
            break;
        }
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
    cancel_token4.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// Record to a buffer for later conversion (non-realtime)
async fn start_recording(input_device: audio::DeviceType, receiver: std::sync::mpsc::Receiver<(bool, f32)>) -> Result<Vec<f32>, AppError> {
    info!("Recording started from {}", input_device);
    let vec = audio::record_audio_vec(input_device, receiver)?;
    Ok(vec)
}

/// Play back the input buffer
async fn play_source(source: Vec<f32>, output_device: audio::DeviceType) -> Result<(), AppError> {
    info!("Playing source audio on {}", output_device);
    debug!("source first 20 samples {:?}", source[0..20].to_vec());
    audio::play_audio_buf(output_device, source, unsafe { OUTPUT_VOLUME })
}

/// Convert and play back the input buffer with the target voice
async fn play_target(source: Vec<f32>, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, vad: vad::Vad, output_device: audio::DeviceType) -> Result<(), AppError> {
    //TODO some kind of loading spinner or something while this is happening
    info!("Running inference and playing target audio on {}", output_device);
    debug!("first 20 recorded samples: {:?}", source[..20].to_vec());

    match output_device {
        DeviceType::File((ref f, _)) => {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: MODEL_SAMPLE_RATE as u32,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
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
                    writer.write_sample(audio::apply_volume(*sample, unsafe { OUTPUT_VOLUME })).map_err(|e| AppError::HoundError(e))?;
                }
            }
            writer.finalize().map_err(|e| AppError::HoundError(e))?;
        },
        DeviceType::Device(ref d) => {
            let playback_secs = source.len() as f32 / 16000.0;

            let output_config = d.default_output_config().map_err(|_e| AppError::BuildOutputStreamError(cpal::BuildStreamError::StreamConfigNotSupported))?.config();
            let out_rate = output_config.sample_rate.0 as usize;
            let n_channels = output_config.channels as usize;
            let mut resampler = FftFixedInOut::<f32>::new(MODEL_SAMPLE_RATE, out_rate, OFFLINE_CHUNK_SIZE, 1).map_err(|e| AppError::ResamplerConstructionError(e))?;
            let mut resampler_input_buffer = resampler.input_buffer_allocate();
            let mut resampler_output_buffer = resampler.output_buffer_allocate();
            let resampler_input_size = resampler.input_frames_next();
            let resampler_output_size = resampler.output_frames_next();
            debug!("out_n_channels: {}", n_channels);
            debug!("out_rate: {}", out_rate);
            debug!("resampler_input_size: {}", resampler_input_size);
            debug!("resampler_output_size: {}", resampler_output_size);

            resampler_input_buffer[0].resize(resampler_input_size, 0.0);

            let output_chunk_size = out_rate * resampler_input_size / MODEL_SAMPLE_RATE * n_channels;
            let ring: RingBuffer<f32> = RingBuffer::new(output_chunk_size*2);
            let (mut producer, mut consumer) = ring.split();
            let ring2: RingBuffer<f32> = RingBuffer::new(resampler_input_size*2);
            let (mut producer2, mut consumer2) = ring2.split();


            let arc_cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel_token1 = arc_cancel_token.clone();
            let cancel_token2 = arc_cancel_token.clone();
            let cancel_token3 = arc_cancel_token.clone();

            std::thread::spawn(move || {
            for chunk in source.chunks(resampler_input_size)
            {
                let chunk_start_time = std::time::Instant::now(); 
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
                    Ok(mut samples) => {
                        // pad for the last chunk so we don't chop off the end of the audio
                        if samples.len() < resampler_input_size {
                            samples.resize(resampler_input_size, 0.0);
                        }
                        debug!("first 20 samples from inference: {:?}", samples[..20].to_vec()); 
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
                        })
                        .for_each(|sample| { producer2.push(sample).unwrap_or(()); });
                    },
                    Err(e) => {
                        // don't spam errors if there's already one known
                        if !cancel_token1.load(std::sync::atomic::Ordering::Relaxed) {
                            handle_errors(Err(e));
                        }
                        cancel_token1.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                let chunk_end_time = std::time::Instant::now();

                let chunk_time = chunk_end_time - chunk_start_time;

                // sleep to prevent ringbuffer overflows
                let sleep_time_secs = resampler_input_size as f32 / MODEL_SAMPLE_RATE as f32 - chunk_time.as_secs_f32() - 0.1;
                if sleep_time_secs > 0.0 {
                    std::thread::sleep(core::time::Duration::from_secs_f32(sleep_time_secs));
                }
            }
            });
            

            std::thread::spawn(move || {
                loop {
                    if consumer2.len() >= resampler_input_size {
                        // resample
                        consumer2.pop_slice(&mut resampler_input_buffer[0][..resampler_input_size]);
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
                    if cancel_token3.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(core::time::Duration::from_millis(50));
                }
            });

            let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    *sample = audio::apply_volume(consumer.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME });
                }
            };

            let output_stream = d.build_output_stream(&output_config, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e))?;
            output_stream.play().map_err(|e| {error!("Output stream error: {}", e); AppError::PlayStreamError("output", e)})?;
            //add a couple seconds in case we fall behind
            let stop_time = std::time::Instant::now() + core::time::Duration::from_secs_f32(playback_secs + 2.0);
            loop {
                if cancel_token2.load(std::sync::atomic::Ordering::Relaxed) || std::time::Instant::now() > stop_time {
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

