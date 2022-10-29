use crate::Message;
use std::path::PathBuf;
use iced::{Length, Text, Row, Column,  Container, Element, slider::{self, Slider}, pick_list::{self, PickList}, button::{self, Button}, text_input::{self, TextInput}, Space, Color, Rectangle};
use iced_aw::{modal::{self, Modal}, card::{self, Card}};
use iced_native::alignment::{Horizontal, Vertical};

#[derive(Default)]
pub struct ViewState {
    pub login_modal_state: modal::State<LoginModalState>,
    pub fakeyou_model_browse_state: button::State,
    pub model_browse_state: button::State,
    pub record_state: button::State,
    pub play_target_state: button::State,
    pub play_source_state: button::State,
    pub realtime_record_state: button::State,
    pub hifigan_browse_state: button::State,
    pub hubert_browse_state: button::State,
    pub input_browse_state: button::State,
    pub output_browse_state: button::State,
    pub download_button_state: button::State,
    pub model_download_list_refresh_button_state: button::State,
    pub local_model_load_state: button::State,
    pub local_model_refresh_state: button::State,
    pub slider_1_state: slider::State,
    pub slider_2_state: slider::State,
    pub input_device_list_state: pick_list::State<String>,
    pub output_device_list_state: pick_list::State<String>,
    pub model_download_list_state: pick_list::State<crate::api::AppModelDownloadsItem>,
    pub local_model_list_state: pick_list::State<crate::local::LocalModelListItem>,
    pub fakeyou_model_browse_selected: Option<PathBuf>,
    pub model_browse_selected: Option<PathBuf>,
    pub hifigan_browse_selected: Option<PathBuf>,
    pub hubert_browse_selected: Option<PathBuf>,
    pub input_browse_selected: Option<PathBuf>,
    pub input_device_list_selected: Option<String>,
    pub output_device_list_selected: Option<String>,
    pub output_browse_selected: Option<PathBuf>,
    pub model_download_list_selected: Option<crate::api::AppModelDownloadsItem>,
    pub local_model_list_selected: Option<crate::local::LocalModelListItem>,
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

pub fn view<'a> (view_state: &'a mut ViewState, input_device_list_options: &'a Vec<String>, output_device_list_options: &'a Vec<String>, model_download_list_options: &'a Vec<crate::api::AppModelDownloadsItem>, local_model_options: &'a Vec<crate::local::LocalModelListItem>, recording: bool, realtime_recording: bool) -> Element<'a, Message> {
    let empty_pathbuf = PathBuf::from("");

    // Model Downloads
    let mut model_download_elems: Vec<Element<Message>> = vec![];
    model_download_elems
            .push(Text::new("Downloadable Models: ").into());
    model_download_elems.push(
    Row::new().spacing(12).push(
        PickList::new(
            &mut view_state.model_download_list_state,
            model_download_list_options,
            view_state.model_download_list_selected.clone(),
            Message::ModelDownloadListSelected,
        )
        .padding(8)
        .style(style::PickList)
        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
        .width(Length::Units(350))
    ).push(
        Button::new(&mut view_state.model_download_list_refresh_button_state, 
            Text::new("Refresh")
                .horizontal_alignment(Horizontal::Center)
                .vertical_alignment(Vertical::Center))
            .style(style::Button::Grey)
            .height(Length::Units(35))
            .width(Length::Units(90))
            .on_press(Message::ModelDownloadListRefreshPressed)
    )
    .push(
        Button::new(&mut view_state.download_button_state, Text::new("Download")
            .horizontal_alignment(Horizontal::Center)
            .vertical_alignment(Vertical::Center))
        .style(style::Button::Grey)
        .height(Length::Units(35))
        .width(Length::Units(90))

        .on_press(Message::ModelDownloadPressed(view_state.model_download_list_selected.clone()))
    ).into());

    // Local Models
    let mut local_model_elems: Vec<Element<Message>> = vec![];
    local_model_elems.push(
        Text::new("Local Models: ").into());
    local_model_elems.push(
    Row::new().spacing(12).push(
        PickList::new(
            &mut view_state.local_model_list_state,
            local_model_options,
            view_state.local_model_list_selected.clone(),
            Message::LocalModelListSelected,
        )
        .padding(8)
        .style(style::PickList)
        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
        .width(Length::Units(350))
    )
    .push(
        Button::new(&mut view_state.local_model_refresh_state, Text::new("Refresh")
                .horizontal_alignment(Horizontal::Center)
                .vertical_alignment(Vertical::Center))
            .style(style::Button::Grey)
            .height(Length::Units(35))
            .width(Length::Units(90))
            .on_press(Message::LocalModelRefreshPressed)
    )
    .push(
        Button::new(&mut view_state.local_model_load_state, Text::new("Load")
                .horizontal_alignment(Horizontal::Center)
                .vertical_alignment(Vertical::Center))
            .style(style::Button::Grey)
            .height(Length::Units(35))
            .width(Length::Units(90))
            .on_press(Message::LocalModelLoadPressed(view_state.local_model_list_selected.clone()))
    ).into());

    
    // NB: New model selection
    /*let mut fakeyou_model_selection_col_elems: Vec<Element<Message>> = vec![];
    let selected_fakeyou_model_str = if view_state.fakeyou_model_browse_selected.is_none() { "no file selected" } else {view_state.fakeyou_model_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };

    fakeyou_model_selection_col_elems
        .push(Row::new().spacing(5).push(Text::new("Select FakeYou Model File:"))
            .push(
            Text::new(selected_fakeyou_model_str)
                .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                .size(14)
                .color(Color::from_rgb8(191, 134, 138))
        ).into());
    fakeyou_model_selection_col_elems
        .push(
            Row::new().spacing(5)
                .push(
                    Button::new(&mut view_state.fakeyou_model_browse_state, Text::new("Browse").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::FakeYouModelBrowsePressed).style(style::Button::Grey).width(Length::Units(260)).height(Length::Units(35))
                )
            .into()
        );

    let fakeyou_model_selection_row = Column::with_children(fakeyou_model_selection_col_elems).spacing(7);

    // NB: Old model selection
    let mut left_model_selection_col_elems: Vec<Element<Message>> = vec![];
    let mut right_model_selection_col_elems: Vec<Element<Message>> = vec![];
    let selected_model_str = if view_state.model_browse_selected.is_none() { "no file selected" } else {view_state.model_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };
    let selected_hifigan_str = if view_state.hifigan_browse_selected.is_none() { "no file selected" } else {view_state.hifigan_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };
    left_model_selection_col_elems
        .push(
            Row::new().spacing(5)
                .push(
                    Text::new("Select Recast Model File:")
                )
                .push(
                    Text::new(selected_model_str)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .size(14)
                        .color(Color::from_rgb8(191, 134, 138))
                ).into()
        );
    left_model_selection_col_elems.push(
        Button::new(&mut view_state.model_browse_state, Text::new("Browse").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::ModelBrowsePressed).style(style::Button::Grey).width(Length::Units(262)).height(Length::Units(35)).into()
    );
    right_model_selection_col_elems.push(
            Row::new().spacing(5)
                .push(Text::new("Select Vocoder:"))
                .push(Text::new(selected_hifigan_str)
                    .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                    .size(14)
                    .color(Color::from_rgb8(191, 134, 138))
                ).into()
    );
    right_model_selection_col_elems.push(
        Button::new(&mut view_state.hifigan_browse_state, Text::new("Browse").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::HifiganBrowsePressed).style(style::Button::Grey).width(Length::Units(262)).height(Length::Units(35)).into()
    );*/


    let mut left_file_selection_col_elems: Vec<Element<Message>> = vec![];
    let mut right_file_selection_col_elems: Vec<Element<Message>> = vec![];
    let selected_input_file_str = if view_state.input_browse_selected.is_none() { "no file selected" } else {view_state.input_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_name().unwrap_or_default().to_str().unwrap_or_default() };
    // FIXME find a way to change the opacity of these controls without defining them twice
    if view_state.input_device_list_selected == Some(String::from("From file"))
    {
        left_file_selection_col_elems.push(
            Row::new().spacing(5)
                .push(
                    Text::new("Input File:").color(Color::from_rgba8(255, 255, 255, 1.0))
                )
                .push(
                    Text::new(selected_input_file_str)
                        .width(Length::Units(200))
                        .height(Length::Units(12))
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .size(12)
                        .color(Color::from_rgba8(191, 134, 138, 1.0))
                )
            .into()
        );
        left_file_selection_col_elems.push(
            Button::new(
                &mut view_state.input_browse_state,
                Text::new("Browse")
                    .color(Color::from_rgba8(255, 255, 255, 1.0))
                    .horizontal_alignment(Horizontal::Center)
                    .vertical_alignment(Vertical::Center)
                )
                .on_press(Message::InputBrowsePressed)
                .style(style::Button::Grey)
                .width(Length::Units(262))
                .height(Length::Units(35))
                .into()
        );

    }
    else {
        left_file_selection_col_elems.push(
            Row::new().spacing(5)
                .push(
                    Text::new("Input File:").color(Color::from_rgba8(255, 255, 255, 0.30))
                )
                .push(
                    Text::new(selected_input_file_str)
                        .width(Length::Units(200))
                        .height(Length::Units(12))
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .size(12)
                        .color(Color::from_rgba8(191, 134, 138, 0.30))
                )
            .into()
        );
        left_file_selection_col_elems.push(
            Button::new(
                &mut view_state.input_browse_state,
                Text::new("Browse")
                    .color(Color::from_rgba8(255, 255, 255, 0.30))
                    .horizontal_alignment(Horizontal::Center)
                    .vertical_alignment(Vertical::Center)
                )
                .style(style::Button::GreyDisabled)
                .width(Length::Units(262))
                .height(Length::Units(35))
                .into()
        );
    }

        let selected_output_file_str = if view_state.output_browse_selected.is_none() { "no file selected" } else {view_state.output_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_name().unwrap_or_default().to_str().unwrap_or_default() };

    if view_state.output_device_list_selected == Some(String::from("To file"))
    {
        right_file_selection_col_elems.push(
            Row::new().spacing(5)
                .push(
                    Text::new("Output File:").color(Color::from_rgba8(255, 255, 255, 1.0))
                )
                .push(
                    Text::new(selected_output_file_str)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .width(Length::Units(186))
                        .height(Length::Units(12))
                        .size(12)
                        .color(Color::from_rgba8(191, 134, 138, 1.0))
                )
            .into()
        );
        right_file_selection_col_elems.push(
            Button::new(&mut view_state.output_browse_state,
                Text::new("Browse")
                    .horizontal_alignment(Horizontal::Center)
                    .vertical_alignment(Vertical::Center))
                    .on_press(Message::OutputBrowsePressed)
                    .style(style::Button::Grey)
                    .width(Length::Units(262))
                    .height(Length::Units(35)).into()
        );
    } else {
        right_file_selection_col_elems.push(
            Row::new().spacing(5)
                .push(
                    Text::new("Output File:").color(Color::from_rgba8(255, 255, 255, 0.30))
                )
                .push(
                    Text::new(selected_output_file_str)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .width(Length::Units(186))
                        .height(Length::Units(12))
                        .size(12)
                        .color(Color::from_rgba8(191, 134, 138, 0.30))
                )
            .into()
        );
        right_file_selection_col_elems.push(
            Button::new(&mut view_state.output_browse_state,
                Text::new("Browse")
                    .color(Color::from_rgba8(255, 255, 255, 0.30))
                    .horizontal_alignment(Horizontal::Center)
                    .vertical_alignment(Vertical::Center))
                    .style(style::Button::GreyDisabled)
                    .width(Length::Units(262))
                    .height(Length::Units(35)).into()
        );
    }

    let main_container = Container::new(
        Column::new().spacing(7)
        .push(Column::with_children(model_download_elems).spacing(10))
        .push(Space::with_height(Length::Units(17)))
        .push(Column::with_children(local_model_elems).spacing(10))
        //.push(fakeyou_model_selection_row)
        .push(Space::with_height(Length::Units(17)))
        .push(
            Row::new().spacing(30).push( 
                Column::new().spacing(7)
                    //.push(Column::with_children(left_model_selection_col_elems).spacing(7))
                    .push(Text::new("Input Device:"))
                    .push(
                        PickList::new(
                            &mut view_state.input_device_list_state,
                            input_device_list_options,
                            view_state.input_device_list_selected.clone(),
                            Message::InputDeviceChanged,
                        )
                        .placeholder("Default Input Device")
                        .style(style::PickList)
                        .padding(8)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .width(Length::Units(262))
                    )
                    .push(Space::with_height(Length::Units(17)))
                    .push(
                        Column::with_children(left_file_selection_col_elems).spacing(7)
                    )
                    .push(Space::with_height(Length::Units(17)))
                    .push(Text::new("Input Volume:"))
                    .push(Slider::new(&mut view_state.slider_1_state, 0.0..=100.0, view_state.slider_1_value, Message::Slider1Changed)
                        .style(style::Slider)
                        .width(Length::Units(262))
                    ) 
            ).push(
                Column::new().spacing(7)
                    //.push(Column::with_children(right_model_selection_col_elems).spacing(7))
                    .push(Text::new("Output Device:"))
                    .push(
                        PickList::new(
                            &mut view_state.output_device_list_state,
                            output_device_list_options,
                            view_state.output_device_list_selected.clone(),
                            Message::OutputDeviceChanged,
                        )
                        .placeholder("Default Output Device")
                        .style(style::PickList)
                        .padding(8)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .width(Length::Units(262))
                    )
                    .push(Space::with_height(Length::Units(17)))
                    .push(
                        Column::with_children(right_file_selection_col_elems).spacing(7)
                    )
                    .push(Space::with_height(Length::Units(17)))
                    .push(Text::new("Output Volume:"))
                    .push(Slider::new(&mut view_state.slider_2_state, 0.0..=100.0, view_state.slider_2_value, Message::Slider2Changed)
                        .style(style::Slider).width(Length::Units(262))
                    )
            )
        )
        .push(Space::with_height(Length::Units(42)))
    ).style(style::MainContainer).width(Length::Fill).height(Length::Fill).padding(28);


    let main_button_container = Container::new(
        Column::new().push(
            Space::with_height(Length::Units(10))
        )
        .push(
            Row::new().spacing(10)
            .push(
                Button::new(&mut view_state.record_state, 
                    if view_state.input_device_list_selected == Some(String::from("From file")) { Text::new("Read File").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center) }
                    else if recording { Text::new("Stop")}
                    else {Text::new("Record").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)}).on_press(Message::RecordPressed)
                    .style(style::Button::Red)
                    .width(Length::Units(130))
                    .height(Length::Units(35))
            )
            .push(
                Button::new(&mut view_state.play_source_state, Text::new("Play Source").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::PlaySourcePressed)
                .style(style::Button::Red)
                .width(Length::Units(130))
                .height(Length::Units(35))
            )
            .push(
                Button::new(&mut view_state.play_target_state, 
                    if view_state.output_device_list_selected == Some(String::from("To file")) { Text::new("Write File").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)}
                    else {
                        Text::new("Play Target").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)
                    })
                    .on_press(Message::PlayTargetPressed)
                    .style(style::Button::Red)
                    .width(Length::Units(130))
                    .height(Length::Units(35))
            )
            .push(
                Button::new(&mut view_state.realtime_record_state, if realtime_recording { Text::new("Stop") } else {Text::new("Realtime").horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)}).on_press(Message::RealtimeRecordPressed)
                .style(style::Button::Red)
                .width(Length::Units(130))
                .height(Length::Units(35))
            )
        )
        .push(Space::with_height(Length::Units(20)))
        .push(Container::new(
                Text::new("FakeYou Recast v1.0").size(12).color(Color::from_rgba8(255, 255, 255, 0.30))
                .font(iced::Font::External { name: "Fira Sans SemiBold", bytes: &style::BOLD_FONT})
        ).width(Length::Fill).center_x())
    ).style(style::MainButtonContainer).width(Length::Fill).height(Length::Units(130)).align_y(Vertical::Bottom).padding(28);

    let main_ui = Column::new().push(main_container).push(main_button_container);

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

pub mod style {
    use iced::{Color, Background, widget::{container, button, pick_list, slider}};
    use iced_native::overlay::menu;
    
    pub struct MainContainer;
    pub struct MainButtonContainer;

    impl container::StyleSheet for MainContainer {
        fn style(&self) -> container::Style {
            container::Style {
                text_color: Some(Color::WHITE),
                background: Some(Background::Color(Color::from_rgb8(26, 26, 39))),
                ..container::Style::default()

            }
        }
    }

    impl container::StyleSheet for MainButtonContainer {
        fn style(&self) -> container::Style {
            container::Style {
                text_color: Some(Color::WHITE),
                background: Some(Background::Color(Color::from_rgb8(36, 36, 51))),
                ..container::Style::default()

            }
        }
    }

    pub enum Button {
        Red,
        RedDisabled,
        Grey,
        GreyDisabled,
    }

    impl button::StyleSheet for Button {
        fn active(&self) -> button::Style {
            match self {
                Button::Red => {
                    button::Style {
                        text_color: Color::WHITE,
                        background: Some(Background::Color(Color::from_rgb8(252, 107, 104))),
                        border_radius: 5.0,
                        ..button::Style::default()
                    }
                },
                Button::Grey => {
                    button::Style {
                    text_color: Color::WHITE,
                        background: Some(Background::Color(Color::from_rgb8(57, 57, 76))),
                        border_radius: 5.0,
                        ..button::Style::default()
                    }
                }
                Button::RedDisabled => { 
                    button::Style {
                        text_color: Color::WHITE,
                        background: Some(Background::Color(Color::from_rgba8(252, 107, 104, 0.30))),
                        border_radius: 5.0,
                        ..button::Style::default()
                    }
                }
                Button::GreyDisabled => {
                    button::Style {
                    text_color: Color::WHITE,
                        background: Some(Background::Color(Color::from_rgba8(57, 57, 76, 0.30))),
                        border_radius: 5.0,
                        ..button::Style::default()
                    }
                }
            }
        }
    }

    pub struct PickList;
    impl PickList {
        fn style() -> pick_list::Style {
           pick_list::Style {
                text_color: Color::WHITE,
                background: Background::Color(Color::from_rgb8(57, 57, 76)),
                border_width: 0.0,
                border_radius: 5.0,
                placeholder_color: Color::WHITE,
                ..Default::default()
           }
        }
    }
    impl pick_list::StyleSheet for PickList {
        fn menu(&self) -> menu::Style {
            menu::Style {
                text_color: Color::WHITE,
                background: Background::Color(Color::from_rgb8(57, 57, 76)),
                selected_background: Background::Color(Color::from_rgb8(90, 90, 120)),
                border_width: 0.0,
                ..Default::default()
            }
        }

        fn active(&self) -> pick_list::Style {
            PickList::style()
        }

        fn hovered(&self) -> pick_list::Style {
            PickList::style()
        }
    }

    pub struct Slider;
    impl Slider {
        fn style() -> slider::Style {
            slider::Style {
                rail_colors: (Color::from_rgb8(137, 65, 69), Color::from_rgb8(137,65,69)),
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle { radius: 8.0 },
                    color: Color::from_rgb8(252, 107, 104),
                    border_color: Color::TRANSPARENT,
                    border_width: 0.0
                },
            }
        }
    }

    impl slider::StyleSheet for Slider {
        fn active(&self) -> slider::Style {
            Slider::style()
        }

        fn hovered(&self) -> slider::Style {
            Slider::style()
        }

        fn dragging(&self) -> slider::Style {
            Slider::style()
        }
    }

    pub const BOLD_FONT: [u8; 473160] = *include_bytes!("../../../../assets/FiraSans-SemiBold.ttf");
    pub const NONBOLD_FONT: [u8; 440984] = *include_bytes!("../../../../assets/FiraSans-Regular.ttf");
}

fn username_text_handler(input: String) -> Message {
    Message::UsernameChanged(input)
}

fn password_text_handler(input: String) -> Message {
    Message::PasswordChanged(input)
}
