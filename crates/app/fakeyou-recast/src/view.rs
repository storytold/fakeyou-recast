use crate::{Message, app_state::AppState};
use std::path::PathBuf;
use iced::{Length, Text, Row, Column,  Container, Element, slider::{self, Slider}, pick_list::{self, PickList}, button::{self, Button}, text_input::{self, TextInput}, Space, Color, Rectangle};
use iced_aw::{modal::{self, Modal}, card::{self, Card}, tab_bar::{TabLabel, TabBar}};
use iced_native::alignment::{Horizontal, Vertical};
use iced_native::{widget::image::Image, image::Handle};
use rust_i18n::t;

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
    pub slider_1_state: slider::State,
    pub slider_2_state: slider::State,
    pub input_device_list_state: pick_list::State<String>,
    pub output_device_list_state: pick_list::State<String>,
    pub fakeyou_model_browse_selected: Option<PathBuf>,
    pub model_browse_selected: Option<PathBuf>,
    pub hifigan_browse_selected: Option<PathBuf>,
    pub hubert_browse_selected: Option<PathBuf>,
    pub input_browse_selected: Option<PathBuf>,
    pub input_device_list_selected: Option<String>,
    pub output_device_list_selected: Option<String>,
    pub output_browse_selected: Option<PathBuf>,
    pub slider_1_value: f32,
    pub slider_2_value: f32,
    pub show_spinner: bool,
    pub spinner_frame: Vec<u8>,
    pub spinner_frame_index: u8,
    pub show_debug: bool,
    pub status_message: String,
    pub active_tab: usize,
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
pub fn view<'a> (view_state: &'a mut ViewState, input_device_list_options: &'a Vec<String>, output_device_list_options: &'a Vec<String>, state: AppState) -> Element<'a, Message> {
    let empty_pathbuf = PathBuf::from("");

    // NB: New model selection
    let mut fakeyou_model_selection_col_elems: Vec<Element<Message>> = vec![];
    let selected_fakeyou_model_str = if view_state.fakeyou_model_browse_selected.is_none() { t!("main_ui.label.no_file") } else {view_state.fakeyou_model_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default().to_string()};

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
                    Button::new(&mut view_state.fakeyou_model_browse_state, Text::new(t!("main_ui.button.browse")).horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::FakeYouModelBrowsePressed).style(style::Button::Grey).width(Length::Units(260)).height(Length::Units(35))
                )
            .into()
        );

    let fakeyou_model_selection_row = Column::with_children(fakeyou_model_selection_col_elems).spacing(7);

    // NB: Old model selection
    let mut left_model_selection_col_elems: Vec<Element<Message>> = vec![];
    let mut right_model_selection_col_elems: Vec<Element<Message>> = vec![];
    let selected_model_str = if view_state.model_browse_selected.is_none() { t!("main_ui.label.no_file") } else {view_state.model_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default().to_string()};
    let selected_hifigan_str = if view_state.hifigan_browse_selected.is_none() { t!("main_ui.label.no_file") } else {view_state.hifigan_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default().to_string()};
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
        Button::new(&mut view_state.model_browse_state, Text::new(t!("main_ui.button.browse")).horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::ModelBrowsePressed).style(style::Button::Grey).width(Length::Units(260)).height(Length::Units(35)).into()
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
        Button::new(&mut view_state.hifigan_browse_state, Text::new(t!("main_ui.button.browse")).horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::HifiganBrowsePressed).style(style::Button::Grey).width(Length::Units(260)).height(Length::Units(35)).into()
    );


    let mut left_file_selection_col_elems: Vec<Element<Message>> = vec![];
    let mut right_file_selection_col_elems: Vec<Element<Message>> = vec![];
    let selected_input_file_str = if view_state.input_browse_selected.is_none() { t!("main_ui.label.no_file")} else {view_state.input_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_name().unwrap_or_default().to_str().unwrap_or_default().to_string()};
    // FIXME find a way to change the opacity of these controls without defining them twice
    if view_state.input_device_list_selected == Some(t!("main_ui.list_item.from_file"))
    {
        left_file_selection_col_elems.push(
            Row::new().spacing(5)
                .push(
                    Text::new(t!("main_ui.label.input_file")).color(Color::from_rgba8(255, 255, 255, 1.0))
                )
                .push(
                    Text::new(selected_input_file_str)
                        .width(Length::Units(200))
                        .height(Length::Units(14))
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .size(14)
                        .color(Color::from_rgba8(191, 134, 138, 1.0))
                )
            .into()
        );
        left_file_selection_col_elems.push(
            Button::new(
                &mut view_state.input_browse_state,
                Text::new(t!("main_ui.button.browse"))
                    .color(Color::from_rgba8(255, 255, 255, 1.0))
                    .horizontal_alignment(Horizontal::Center)
                    .vertical_alignment(Vertical::Center)
                )
                .on_press(Message::InputBrowsePressed)
                .style(style::Button::Grey)
                .width(Length::Units(260))
                .height(Length::Units(35))
                .into()
        );

    }
    else {
        left_file_selection_col_elems.push(
            Row::new().spacing(5)
                .push(
                    Text::new(t!("main_ui.label.input_file")).color(Color::from_rgba8(255, 255, 255, 0.30))
                )
                .push(
                    Text::new(selected_input_file_str)
                        .width(Length::Units(200))
                        .height(Length::Units(14))
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .size(14)
                        .color(Color::from_rgba8(191, 134, 138, 0.30))
                )
            .into()
        );
        left_file_selection_col_elems.push(
            Button::new(
                &mut view_state.input_browse_state,
                Text::new(t!("main_ui.button.browse"))
                    .color(Color::from_rgba8(255, 255, 255, 0.30))
                    .horizontal_alignment(Horizontal::Center)
                    .vertical_alignment(Vertical::Center)
                )
                .style(style::Button::GreyDisabled)
                .width(Length::Units(260))
                .height(Length::Units(35))
                .into()
        );
    }

        let selected_output_file_str = if view_state.output_browse_selected.is_none() { t!("main_ui.label.no_file")} else {view_state.output_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_name().unwrap_or_default().to_str().unwrap_or_default().to_string()};

    if view_state.output_device_list_selected == Some(t!("main_ui.list_item.to_file"))
    {
        right_file_selection_col_elems.push(
            Row::new().spacing(5)
                .push(
                    Text::new(t!("main_ui.label.output_file")).color(Color::from_rgba8(255, 255, 255, 1.0))
                )
                .push(
                    Text::new(selected_output_file_str)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .width(Length::Units(186))
                        .height(Length::Units(14))

                        .size(14)
                        .color(Color::from_rgba8(191, 134, 138, 1.0))
                )
            .into()
        );
        right_file_selection_col_elems.push(
            Button::new(&mut view_state.output_browse_state,
                Text::new(t!("main_ui.button.browse"))
                    .horizontal_alignment(Horizontal::Center)
                    .vertical_alignment(Vertical::Center))
                    .on_press(Message::OutputBrowsePressed)
                    .style(style::Button::Grey)
                    .width(Length::Units(260))
                    .height(Length::Units(35)).into()
        );
    } else {
        right_file_selection_col_elems.push(
            Row::new().spacing(5)
                .push(
                    Text::new(t!("main_ui.label.output_file")).color(Color::from_rgba8(255, 255, 255, 0.30))
                )
                .push(
                    Text::new(selected_output_file_str)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .width(Length::Units(186))
                        .height(Length::Units(14))
                        .size(14)
                        .color(Color::from_rgba8(191, 134, 138, 0.30))
                )
            .into()
        );
        right_file_selection_col_elems.push(
            Button::new(&mut view_state.output_browse_state,
                Text::new(t!("main_ui.button.browse"))
                    .color(Color::from_rgba8(255, 255, 255, 0.30))
                    .horizontal_alignment(Horizontal::Center)
                    .vertical_alignment(Vertical::Center))
                    .style(style::Button::GreyDisabled)
                    .width(Length::Units(260))
                    .height(Length::Units(35)).into()
        );
    }

    let spinner_image_handle = Handle::from_memory(view_state.spinner_frame.clone());
    let spinner = if view_state.show_spinner {
        Container::new(Image::<Handle>::new(spinner_image_handle).height(Length::Units(50))).height(Length::Fill).align_y(Vertical::Bottom).width(Length::Fill).center_x()
    } else {
        Container::new(Space::with_height(Length::Units(0))) // FIXME janky empty widget
    };

    let main_container = Container::new(
        Column::new().spacing(7)
        .push(fakeyou_model_selection_row)
        .push(
            Row::new().spacing(33).push( 
                Column::new().spacing(7)
                    .push(Column::with_children(left_model_selection_col_elems).spacing(7))
                    .push(Text::new(t!("main_ui.label.input_dev")))
                    .push(
                        PickList::new(
                            &mut view_state.input_device_list_state,
                            input_device_list_options,
                            view_state.input_device_list_selected.clone(),
                            Message::InputDeviceChanged,
                        )
                        .placeholder(t!("main_ui.list_item.input_default"))
                        .style(style::PickList)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .width(Length::Units(260))
                    )
                    .push(
                        Column::with_children(left_file_selection_col_elems).spacing(7)
                    )
                    .push(Text::new(t!("main_ui.label.input_vol")))
                    .push(
                        Container::new(
                            Slider::new(&mut view_state.slider_1_state, 0.0..=100.0, view_state.slider_1_value, Message::Slider1Changed).style(style::Slider).height(44)
                        ).width(Length::Units(260))
                    ) 
            ).push(
                Column::new().spacing(7)
                    .push(Column::with_children(right_model_selection_col_elems).spacing(7))
                    .push(Text::new(t!("main_ui.label.output_dev")))
                    .push(
                        PickList::new(
                            &mut view_state.output_device_list_state,
                            output_device_list_options,
                            view_state.output_device_list_selected.clone(),
                            Message::OutputDeviceChanged,
                        )
                        .placeholder(t!("main_ui.list_item.output_default"))
                        .style(style::PickList)
                        .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                        .width(Length::Units(260))
                    )
                    .push(
                        Column::with_children(right_file_selection_col_elems).spacing(7)
                    )
                    .push(Text::new(t!("main_ui.label.output_vol")))
                    .push(
                        Container::new(
                            Slider::new(&mut view_state.slider_2_state, 0.0..=100.0, view_state.slider_2_value, Message::Slider2Changed).style(style::Slider).height(44)
                        ).width(Length::Units(260))
                    )
            )
        )
        .push(Space::with_height(Length::Units(42)))
        .push(Container::new(
                Text::new(view_state.status_message.clone())
                    .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
                ).width(Length::Fill)
            .center_x()
        )
        .push(spinner)
    ).style(style::MainContainer).width(Length::Fill).height(Length::Fill).padding(28);


    let main_button_container = Container::new(
        Column::new().push(
            Space::with_height(Length::Units(10))
        )
        .push(
            Row::new().spacing(10)
            .push(
                Button::new(&mut view_state.record_state, 
                    Text::new( if view_state.input_device_list_selected == Some(t!("main_ui.list_item.from_file")) { t!("main_ui.button.read_file") } else if state == AppState::Recording || state == AppState::ReadingFile { t!("main_ui.button.stop") } else { t!("main_ui.button.record")} ).horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center))
                    .on_press(Message::RecordPressed)
                    .style(style::Button::Red)
                    .width(Length::Units(130))
                    .height(Length::Units(35))
            )
            .push(
                Button::new(&mut view_state.play_source_state, Text::new(if state == AppState::PlaySource { t!("main_ui.button.stop") } else { t!("main_ui.button.play_source")}).horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::PlaySourcePressed)
                .style(style::Button::Red)
                .width(Length::Units(130))
                .height(Length::Units(35))
            )
            .push(
                Button::new(&mut view_state.play_target_state, 
                    Text::new(if view_state.output_device_list_selected == Some(t!("main_ui.list_item.to_file")) {t!("main_ui.button.write_file")} else if state == AppState::PlayTarget || state == AppState::WritingFile { t!("main_ui.button.stop") } else { t!("main_ui.button.play_target")}).horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center))
                    .on_press(Message::PlayTargetPressed)
                    .style(style::Button::Red)
                    .width(Length::Units(130))
                    .height(Length::Units(35))
            )
            .push(
                Button::new(&mut view_state.realtime_record_state, Text::new(if state == AppState::Realtime { t!("main_ui.button.stop") } else { t!("main_ui.button.realtime")}).horizontal_alignment(Horizontal::Center).vertical_alignment(Vertical::Center)).on_press(Message::RealtimeRecordPressed)
                .style(style::Button::Red)
                .width(Length::Units(130))
                .height(Length::Units(35))
            )
        )
        .push(Space::with_height(Length::Units(20)))
        .push(Container::new(
                Text::new(t!("app_name") + " v1.0").size(12).color(Color::from_rgba8(255, 255, 255, 0.30))
                .font(iced::Font::External { name: "Fira Sans Regular", bytes: &style::NONBOLD_FONT})
        ).width(Length::Fill).center_x())
    ).style(style::MainButtonContainer).width(Length::Fill).height(Length::Units(130)).align_y(Vertical::Bottom).padding(28);

    let main_tab = Column::new().push(main_container).push(main_button_container);
    //FIXME
    let news_tab = Column::new().push(Container::new(Column::new()).style(style::MainContainer).width(Length::Fill).height(Length::Fill));
    let settings_tab = Column::new().push(Container::new(Column::new()).style(style::MainContainer).width(Length::Fill).height(Length::Fill));

    let  main_ui = Column::new().push(
        TabBar::width_tab_labels(view_state.active_tab, 
            vec![
                TabLabel::Text(String::from("Main")),
                TabLabel::Text(String::from("News")),
                TabLabel::Text(String::from("Settings"))
            ], |active_tab|
            {
                Message::TabChanged(active_tab)
            }
        )
    ).push(
        match view_state.active_tab {
            0 => main_tab,
            1 => news_tab,
            2 => settings_tab,
            _ => unreachable!()
        }
    );


    // Login Modal
    Modal::new(&mut view_state.login_modal_state, main_ui, |state| 
       Card::new(
           Text::new(t!("login_modal.label.login")),
           Column::new().spacing(10)
           .push(
                Row::new().spacing(5)
                .push(Text::new(t!("login_modal.label.username")))
                .push(
                    Container::new(
                        TextInput::new(&mut state.username_field_state, "", &mut state.username_field_value, username_text_handler).on_submit(Message::LoginPressed)
                    ).width(iced::Length::Units(140)).align_x(iced::alignment::Horizontal::Right)
                )
           )
           .push(
                Row::new().spacing(8) // bad hack to make the input fields line up despite
                                      // differing widths of text 'username' and 'password'
                .push(Text::new(t!("login_modal.label.password")))
                .push(
                    Container::new(
                        TextInput::new(&mut state.password_field_state, "", &mut state.password_field_value, password_text_handler).password().on_submit(Message::LoginPressed)
                    ).width(iced::Length::Units(140)).align_x(iced::alignment::Horizontal::Right)
                )
            )
           .push(Text::new(if state.login_failed { &state.login_err_msg } else { "" }).color(iced::Color::from_rgb(1.0, 0.0, 0.0)).size(15))
        ).foot(
            Container::new(
            Button::new(&mut state.login_button_state, Text::new(t!("login_modal.label.login"))).on_press(Message::LoginPressed)
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
