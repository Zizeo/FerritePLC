# AGENT INSTRUCTION: EGUI TO ICED UI/UX MIGRATION & MODERNIZATION



You are an expert Rust GUI engineer and modern UI/UX designer. Your task is to rewrite and modernize an existing `egui` desktop application using `iced` (https://github.com/iced-rs/iced).



The primary objectives are:

1. Architectural Transition: Convert the application from egui's immediate-mode update loop to Iced's declarative Elm Architecture (State / Message / Update / View).

2. Visual Modernization: Redesign the interface to feel crisp, spacious, and contemporary (inspired by sleek modern apps like Linear, Raycast, and VS Code), avoiding default or plain styling.



---



## 1. CORE ARCHITECTURAL TRANSLATION RULES



### A. Immediate Mode vs. The Elm Architecture (TEA)

- egui pattern: State is mutated directly within `eframe::App::update(&mut self, ctx, _frame)` every frame.

- iced pattern: Strict unidirectional data flow:

  - `State`: Holds only clean application data (no rendering logic).

  - `Message`: Strongly typed enum representing every user interaction and async event.

  - `update(&mut self, message: Message) -> Task<Message>`: Handles state mutations and side effects cleanly.

  - `view(&self) -> Element<'_, Message>`: Pure function describing the UI tree.

  - `subscription(&self) -> Subscription<Message>`: Handles timers, window events, or external streams.



### B. Widget & Layout Mapping

- Translate panels:

  - `egui::TopBottomPanel::top` -> Header `container(row![...])` with full width, fixed height, and subtle border/bottom border.

  - `egui::SidePanel::left` -> Sidebar `container(column![...])` with fixed width, vertical fill, and subtle right border.

  - `egui::CentralPanel` -> Main content `container(scrollable(column![...]))` set to `width(Length::Fill)` and `height(Length::Fill)`.

- Translate layout containers:

  - `ui.horizontal(...)` -> `row![...].spacing(...)`

  - `ui.vertical(...)` -> `column![...].spacing(...)`

  - `ui.scroll_area(...)` -> `scrollable(...)`

  - `egui::Grid` -> Composed `row!` and `column!` flex structures or `iced::widget::responsive`.

- Replace inputs & controls:

  - `ui.text_edit_singleline(&mut text)` -> `text_input("Placeholder...", &self.text).on_input(Message::TextChanged)`

  - `ui.checkbox(&mut bool, ...)` -> `toggler(self.flag).label(...).on_toggle(Message::FlagToggled)`

  - `ui.button(...)` -> `button(...).on_press(Message::Action)`

  - `egui::ComboBox` -> `pick_list(&self.options, self.selected, Message::Selected)`



---



## 2. MODERN VISUAL DESIGN SYSTEM (AESTHETIC SPECIFICATION)



Implement a cohesive, modern visual language using Iced's styling API and custom themes.



### A. Color Palette & Hierarchy

- Dark Theme (Default):

  - Canvas / Window Base: Deep dark background (`#0B0D11` or `#111318`).

  - Elevated Surfaces / Cards / Sidebar: `#171A21` or `#1E222B`.

  - Subtle Surface / Hover: `#262B36`.

  - Borders & Outlines: 1px hairline borders (`rgba(255, 255, 255, 0.08)` or `#2D323F`).

  - Accent / Primary Action: Vibrant indigo/blue (`#6366F1` or `#3B82F6`).

  - Typography: Primary (`#F3F4F6`), Secondary (`#9CA3AF`), Disabled/Placeholder (`#4B5563`).



### B. Spacing, Geometry & Typography

- Spacing Rhythm: Standard 4px / 8px scale:

  - Micro gap: 4px–8px.

  - Component padding: 12px–16px.

  - Section / Page padding: 24px–32px.

- Border Radii:

  - Buttons / Inputs: 6px–8px.

  - Cards / Panels / Containers: 10px–14px.

  - Pill tags / badges: 9999px.

- Typography: Use clear visual contrast with distinct font weights (e.g., bold headings, medium labels, regular body).



### C. Container & Button Styling

- Avoid standard plain gray rectangles. Wrap logical sections into styled cards using `container(...)` with custom borders, rounded corners, and elevated backgrounds.

- Provide clear hover and pressed state differentiation on clickable components.



---



## 3. STEP-BY-STEP REBUILD PROCESS



### Step 1: Analyze & Decouple egui Code

1. Inspect the existing `egui` app:

   - Extract state variables into a dedicated application struct.

   - Separate transient UI state (e.g., input drafts, temporary view flags) from core business data.

   - Enumerate all interactions and map them to explicit `Message` enum variants.



### Step 2: Define Theme & Style Helpers

1. Set up a modular `theme.rs` or styling helper functions:

   - Define custom styles for containers (cards, headers, sidebars).

   - Define custom button styles (primary, secondary, danger, ghost).

   - Define custom input styles with clear focus outlines.



### Step 3: Implement Elm Architecture Logic

1. Implement `iced::application` (or `iced::run`):

   - `fn update(&mut self, message: Message) -> Task<Message>`

   - Implement pure state transitions for each `Message`.

   - Dispatch background work or I/O using `Task::perform` or `Task::future`.



### Step 4: Rebuild Views & Layout Hierarchy

1. Create a modern application shell:

   - Top navigation or search bar.

   - Sidebar with styled navigation items (active indicator, hover background).

   - Central view area wrapping feature components in cards and clean scroll regions.

2. Decompose complex screens into small, readable helper functions returning `Element<'_, Message>`.



### Step 5: Refinement & Validation

1. Verify keyboard focus and tab navigation.

2. Ensure responsive window scaling behavior (`Length::Fill`, `Length::Shrink`, `Length::Fixed`).

3. Ensure the project compiles cleanly with zero warnings.



---



## 4. EXAMPLE PATTERN REFERENCE (ICED 0.13+)



Use this idiomatic pattern when constructing the application:



```rust

use iced::widget::{button, column, container, row, scrollable, text, text_input};

use iced::{border, color, Alignment, Background, Border, Color, Element, Length, Task, Theme};



pub struct ModernApp {

    search_query: String,

    active_tab: Tab,

    items: Vec<String>,

}



#[derive(Debug, Clone)]

pub enum Message {

    SearchChanged(String),

    TabSelected(Tab),

    ItemClicked(usize),

}



#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Tab {

    Overview,

    Settings,

}



impl ModernApp {

    pub fn update(&mut self, message: Message) -> Task<Message> {

        match message {

            Message::SearchChanged(val) => {

                self.search_query = val;

                Task::none()

            }

            Message::TabSelected(tab) => {

                self.active_tab = tab;

                Task::none()

            }

            Message::ItemClicked(_idx) => {

                // handle interaction

                Task::none()

            }

        }

    }



    pub fn view(&self) -> Element<'_, Message> {

        let sidebar = container(

            column![

                text("Workspace").size(18),

                button("Overview")

                    .on_press(Message::TabSelected(Tab::Overview))

                    .style(subtle_button_style),

                button("Settings")

                    .on_press(Message::TabSelected(Tab::Settings))

                    .style(subtle_button_style),

            ]

            .spacing(8)

            .padding(16),

        )

        .width(220)

        .height(Length::Fill)

        .style(sidebar_style);



        let main_content = container(

            scrollable(

                column![

                    text_input("Type to search...", &self.search_query)

                        .on_input(Message::SearchChanged)

                        .padding(10)

                        .style(input_style),

                    container(text("Main Card Content").size(15))

                        .padding(20)

                        .width(Length::Fill)

                        .style(card_style),

                ]

                .spacing(16)

                .padding(24),

            )

        )

        .width(Length::Fill)

        .height(Length::Fill);



        row![sidebar, main_content].into()

    }

}



// Styling helpers

fn sidebar_style(_theme: &Theme) -> container::Style {

    container::Style {

        background: Some(Background::Color(Color::from_rgb8(23, 26, 33))),

        border: Border {

            color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),

            width: 1.0,

            radius: border::Radius::default(),

        },

        ..Default::default()

    }

}



fn card_style(_theme: &Theme) -> container::Style {

    container::Style {

        background: Some(Background::Color(Color::from_rgb8(30, 34, 43))),

        border: Border {

            color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),

            width: 1.0,

            radius: border::Radius::from(10.0),

        },

        ..Default::default()

    }

}



fn subtle_button_style(_theme: &Theme, status: button::Status) -> button::Style {

    let bg = match status {

        button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 0.08),

        button::Status::Pressed => Color::from_rgba(1.0, 1.0, 1.0, 0.04),

        _ => Color::TRANSPARENT,

    };

    button::Style {

        background: Some(Background::Color(bg)),

        text_color: Color::from_rgb8(243, 244, 246),

        border: Border {

            radius: border::Radius::from(6.0),

            ..Default::default()

        },

        ..Default::default()

    }

}



fn input_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {

    let border_color = match status {

        text_input::Status::Focused => Color::from_rgb8(99, 102, 241),

        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.1),

    };

    text_input::Style {

        background: Background::Color(Color::from_rgb8(17, 19, 24)),

        border: Border {

            color: border_color,

            width: 1.0,

            radius: border::Radius::from(8.0),

        },

        icon: Color::from_rgb8(156, 163, 175),

        placeholder: Color::from_rgb8(107, 114, 128),

        value: Color::from_rgb8(243, 244, 246),

        selection: Color::from_rgba(99.0 / 255.0, 102.0 / 255.0, 241.0 / 255.0, 0.3),

    }

}

```



---



## 5. INITIATION PROMPT FOR TARGET CODE



When you receive the egui source code:

1. Provide a quick summary of the visual improvements and component breakdown planned.

2. Outline the `Message` enum and Elm Architecture structure.

3. Output the complete, modular Iced source code, replacing the egui implementation entirely.