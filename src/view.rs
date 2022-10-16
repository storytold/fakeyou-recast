use crate::Message;
use std::path::PathBuf;
use iced::{Text, Row, Column, Container, Element, slider::{self, Slider}, pick_list::{self, PickList}, button::{self, Button}, text_input::{self, TextInput}};
use iced_aw::{modal::{self, Modal}, card::{self, Card}};

#[derive(Default)]
pub struct ViewState {
    pub login_modal_state: modal::State<LoginModalState>,
    pub model_browse_state: button::State,
    pub record_state: button::State,
    pub play_target_state: button::State,
    pub play_source_state: button::State,
    pub realtime_record_state: button::State,
    pub hifigan_browse_state: button::State,
    pub hubert_browse_state: button::State,
    pub input_browse_state: button::State,
    pub output_browse_state: button::State,
    pub slider_1_state: slider::State,
    pub slider_2_state: slider::State,
    pub input_device_list_state: pick_list::State<String>,
    pub output_device_list_state: pick_list::State<String>,
    pub model_browse_selected: Option<PathBuf>,
    pub hifigan_browse_selected: Option<PathBuf>,
    pub hubert_browse_selected: Option<PathBuf>,
    pub input_browse_selected: Option<PathBuf>,
    pub input_device_list_selected: Option<String>,
    pub output_device_list_selected: Option<String>,
    pub output_browse_selected: Option<PathBuf>,
    pub slider_1_value: f32,
    pub slider_2_value: f32,
    pub show_debug: bool,
}

#[derive(Default)]
pub struct LoginModalState {
    pub login_button_state: button::State,
    pub username_field_state: text_input::State,
    pub username_field_value: String,
    pub password_field_state: text_input::State,
    pub password_field_value: String,
    pub login_failed: bool,
    pub login_err_msg: String
}

impl ViewState{
    pub fn new() -> Self {
        let mut view_state = Self::default();
        view_state.slider_1_value = 80.0;
        view_state.slider_2_value = 80.0;
        view_state
    }
}
pub fn view<'a> (view_state: &'a mut ViewState, input_device_list_options: &'a Vec<String>, output_device_list_options: &'a Vec<String>, recording: bool, realtime_recording: bool) -> Element<'a, Message> {
    let mut model_selection_row_elems: Vec<Element<Message>> = vec![];
    let empty_pathbuf = PathBuf::from("");
    let selected_model_str = if view_state.model_browse_selected.is_none() { "" } else {view_state.model_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };
    let selected_hifigan_str = if view_state.hifigan_browse_selected.is_none() { "" } else {view_state.hifigan_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };
    let selected_hubert_str = if view_state.hubert_browse_selected.is_none() { "" } else {view_state.hubert_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };
    model_selection_row_elems
        .push(Text::new("Select Recast Model File: ").into());
    model_selection_row_elems
        .push(Button::new(&mut view_state.model_browse_state, Text::new("Browse")).on_press(Message::ModelBrowsePressed).into());
    model_selection_row_elems
        .push(Text::new(selected_model_str).into());
        // TODO don't forget to remove this vocoder button when the models are merged to one file 
        model_selection_row_elems
            .push(Text::new("Select Vocoder: ").into());
        model_selection_row_elems
            .push(Button::new(&mut view_state.hifigan_browse_state, Text::new("Browse")).on_press(Message::HifiganBrowsePressed).into());
        model_selection_row_elems
        .push(Text::new(selected_hifigan_str).into());
    if view_state.show_debug {
        model_selection_row_elems
            .push(Text::new("Select Debug Model: ").into());
        model_selection_row_elems
            .push(Button::new(&mut view_state.hubert_browse_state, Text::new("Browse")).on_press(Message::HubertBrowsePressed).into());
        model_selection_row_elems
        .push(Text::new(selected_hubert_str).into());
    }
    let model_selection_row = Row::with_children(model_selection_row_elems).spacing(10);

    let mut file_selection_row_elems: Vec<Element<Message>> = vec![];
    if view_state.input_device_list_selected == Some(String::from("From file"))
    {
        let selected_input_file_str = if view_state.input_browse_selected.is_none() { "" } else {view_state.input_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_name().unwrap_or_default().to_str().unwrap_or_default() };
        file_selection_row_elems.push(
            Text::new("Input File: ").into()
        );
        file_selection_row_elems.push(
            Button::new(&mut view_state.input_browse_state, Text::new("Browse")).on_press(Message::InputBrowsePressed).into()
        );
        file_selection_row_elems.push(Text::new(selected_input_file_str).into());
    }
    if view_state.output_device_list_selected == Some(String::from("To file"))
    {
        let selected_output_file_str = if view_state.output_browse_selected.is_none() { "" } else {view_state.output_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_name().unwrap_or_default().to_str().unwrap_or_default() };
        file_selection_row_elems.push(
            Text::new("Output File: ").into()
        );
        file_selection_row_elems.push(
            Button::new(&mut view_state.output_browse_state, Text::new("Browse")).on_press(Message::OutputBrowsePressed).into()
        );
        file_selection_row_elems.push(Text::new(selected_output_file_str).into());
    }
    let file_selection_row = Row::with_children(file_selection_row_elems).spacing(10);

    let main_ui = Column::new().spacing(5)
        .push(model_selection_row)
        .push(
            Row::new().spacing(10)
            .push(Text::new("Input Device: "))
            .push(
                PickList::new(
                    &mut view_state.input_device_list_state,
                    input_device_list_options,
                    view_state.input_device_list_selected.clone(),
                    Message::InputDeviceChanged,
                )
            )
            .push(Text::new("Output Device: "))
            .push(
                PickList::new(
                    &mut view_state.output_device_list_state,
                    output_device_list_options,
                    view_state.output_device_list_selected.clone(),
                    Message::OutputDeviceChanged,
                )
            )
        )
        .push(
            file_selection_row
        )
        .push(
            Row::new().spacing(10)
            .push(Text::new("Input Volume: "))
            .push(
                Slider::new(&mut view_state.slider_1_state, 0.0..=100.0, view_state.slider_1_value, Message::Slider1Changed)
            )
            .push(Text::new("Output Volume: "))
            .push(
                Slider::new(&mut view_state.slider_2_state, 0.0..=100.0, view_state.slider_2_value, Message::Slider2Changed)
            )
        )
        .push(
            Row::new().spacing(10)
            .push(
                Button::new(&mut view_state.record_state, 
                    if view_state.input_device_list_selected == Some(String::from("From file")) { Text::new("Read File")  }
                    else if recording { Text::new("Stop")}
                    else {Text::new("Record")}).on_press(Message::RecordPressed)
            )
            .push(
                Button::new(&mut view_state.play_source_state, Text::new("Play Source")).on_press(Message::PlaySourcePressed)
            )
            .push(
                Button::new(&mut view_state.play_target_state, 
                    if view_state.output_device_list_selected == Some(String::from("To file")) { Text::new("Write File") }
                    else {
                        Text::new("Play Target")
                    })
                    .on_press(Message::PlayTargetPressed)
            )
            .push(
                Button::new(&mut view_state.realtime_record_state, if realtime_recording { Text::new("Stop") } else {Text::new("Realtime\n(experimental)")}).on_press(Message::RealtimeRecordPressed)
            )

        );



    // Login Modal
    Modal::new(&mut view_state.login_modal_state, main_ui, |state| 
       Card::new(
           Text::new("Login"),
           Column::new().spacing(10)
           .push(
                Row::new().spacing(5)
                .push(Text::new("Username:"))
                .push(
                    Container::new(
                        TextInput::new(&mut state.username_field_state, "", &mut state.username_field_value, username_text_handler).on_submit(Message::LoginPressed)
                    ).width(iced::Length::Units(140)).align_x(iced::alignment::Horizontal::Right)
                )
           )
           .push(
                Row::new().spacing(8) // bad hack to make the input fields line up despite
                                      // differing widths of text 'username' and 'password'
                .push(Text::new("Password:"))
                .push(
                    Container::new(
                        TextInput::new(&mut state.password_field_state, "", &mut state.password_field_value, password_text_handler).password().on_submit(Message::LoginPressed)
                    ).width(iced::Length::Units(140)).align_x(iced::alignment::Horizontal::Right)
                )
            )
           .push(Text::new(if state.login_failed { &state.login_err_msg } else { "" }).color(iced::Color::from_rgb(1.0, 0.0, 0.0)).size(15))
        ).foot(
            Container::new(
            Button::new(&mut state.login_button_state, Text::new("Login")).on_press(Message::LoginPressed)
            ).width(iced::Length::Fill).center_x()
        )
       .max_width(250)
       .into()
    ).into()
}

fn username_text_handler(input: String) -> Message {
    Message::UsernameChanged(input)
}

fn password_text_handler(input: String) -> Message {
    Message::PasswordChanged(input)
}
