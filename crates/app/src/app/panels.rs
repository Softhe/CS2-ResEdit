use super::*;

impl ResEditApp {
    fn select_account(&mut self, index: usize) {
        let has_config = self.accounts[index].has_config;
        let path = PathBuf::from(&self.accounts[index].config_path);
        if has_config {
            let previous = self.selected_account.replace(index);
            if !self.load_path(Some(path)) {
                self.selected_account = previous;
            }
        } else {
            self.selected_account = Some(index);
            self.load_path(None);
            self.set_status(
                "This account does not have a CS2 video configuration.",
                true,
            );
        }
    }
    pub(super) fn settings_card(&mut self, ui: &mut egui::Ui, min_height: f32) -> egui::Rect {
        card_frame(ui, self.palette).show(ui, |ui| {
            ui.set_min_height((min_height - 22.0).max(0.0));
            ui.heading("Configuration");
            ui.label("Choose an account or browse to a config file.");
            section_rule(ui, self.palette.accent);
            ui.label(egui::RichText::new("STEAM ACCOUNT").small().strong());

            // Steam account row.
            let selected_label = self
                .selected_account
                .and_then(|i| self.accounts.get(i))
                .map(|a| a.display_name.clone())
                .unwrap_or_else(|| "Select an account".to_string());
            let account_combo = egui::ComboBox::from_id_salt("Steam account")
                .selected_text(selected_label.clone())
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    let mut clicked = None;
                    for (index, account) in self.accounts.iter().enumerate() {
                        if ui
                            .selectable_label(
                                self.selected_account == Some(index),
                                &account.display_name,
                            )
                            .clicked()
                        {
                            clicked = Some(index);
                        }
                    }
                    if let Some(index) = clicked {
                        self.select_account(index);
                    }
                });
            named_combo(&account_combo.response, "Steam account", ui.is_enabled());
            if let Some(index) = wheel_selection(&account_combo.response, self.selected_account.unwrap_or(0), self.accounts.len()) {
                self.select_account(index);
            }
            account_combo.response.on_hover_text(selected_label);
            ui.horizontal(|ui| {
                if ui.add_enabled(self.account_scan.is_none(), egui::Button::new("Refresh")).clicked() {
                    self.refresh_accounts();
                }
                if ui.button("Browse").clicked() {
                    self.browse();
                }
                if ui.add_enabled(self.selected_path.is_some(), egui::Button::new("Reload file")).on_hover_text("Read the selected configuration again, including changes made outside CS2 ResEdit").clicked() {
                    self.load_path(self.selected_path.clone());
                }
            });

            let path_label = self
                .selected_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "No valid configuration selected.".to_string());
            ui.add(egui::Label::new(egui::RichText::new(path_label.clone()).monospace().small()).truncate())
                .on_hover_text(path_label);

            ui.separator();
            ui.heading("Display settings");
            ui.label("Aspect mode filters the available preset list.");
            section_rule(ui, self.palette.accent);

            // Target display.
            let display_label = self
                .display_labels
                .get(self.selected_display)
                .cloned()
                .unwrap_or_else(|| "No displays reported by Windows".to_string());
            ui.label(egui::RichText::new("TARGET DISPLAY").small().strong());
            if self.displays.is_empty() {
                ui.label(display_label);
            } else {
                let display_combo = egui::ComboBox::from_id_salt("Target display")
                    .selected_text(display_label.clone())
                    .width(ui.available_width().min(400.0))
                    .truncate()
                    .show_ui(ui, |ui| {
                        let mut clicked = None;
                        for (index, label) in self.display_labels.iter().enumerate() {
                            if ui
                                .selectable_label(self.selected_display == index, label.as_str())
                                .clicked()
                            {
                                clicked = Some(index);
                            }
                        }
                        if let Some(index) = clicked {
                            self.selected_display = index;
                            self.display_changed();
                        }
                    });
                named_combo(&display_combo.response, "Target display", ui.is_enabled());
                if let Some(index) = wheel_selection(&display_combo.response, self.selected_display, self.displays.len()) {
                    self.selected_display = index;
                    self.display_changed();
                }
                display_combo.response.on_hover_text(display_label);
            }

            // Preset + aspect.
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("RESOLUTION PRESET").small().strong());
                    let preset_label = self
                        .preset_choices
                        .get(self.selected_preset)
                        .map(|c| c.label.clone())
                        .unwrap_or_default();
                    let preset_combo = egui::ComboBox::from_id_salt("Resolution preset")
                        .selected_text(preset_label)
                        .show_ui(ui, |ui| {
                            let mut clicked = None;
                            for (index, choice) in self.preset_choices.iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_preset == index, &choice.label)
                                    .clicked()
                                {
                                    clicked = Some(index);
                                }
                            }
                            if let Some(index) = clicked {
                                self.on_preset_selected(index);
                            }
                        });
                    named_combo(&preset_combo.response, "Resolution preset", ui.is_enabled());
                    if let Some(index) = wheel_selection(&preset_combo.response, self.selected_preset, self.preset_choices.len()) {
                        self.on_preset_selected(index);
                    }
                });
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("ASPECT RATIO").small().strong());
                    let aspect_combo = egui::ComboBox::from_id_salt("Aspect ratio")
                        .selected_text(ASPECT_LABELS[self.aspect_index])
                        .show_ui(ui, |ui| {
                            for (index, label) in ASPECT_LABELS.iter().enumerate() {
                                if ui
                                    .selectable_label(self.aspect_index == index, *label)
                                    .clicked()
                                {
                                    self.on_aspect_selected(index);
                                }
                            }
                        });
                    named_combo(&aspect_combo.response, "Aspect ratio", ui.is_enabled());
                    if let Some(index) = wheel_selection(&aspect_combo.response, self.aspect_index, ASPECT_LABELS.len()) {
                        self.on_aspect_selected(index);
                    }
                });
            });

            // Custom dimensions (visible for the Custom preset only).
            if self.selected_preset == 0 {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let width_heading = ui.label(egui::RichText::new("CUSTOM WIDTH").small().strong());
                        let response = ui
                            .add(
                                egui::TextEdit::singleline(&mut self.custom_width)
                                    .hint_text("Custom width")
                                    .desired_width(160.0),
                            ).labelled_by(width_heading.id);
                        if response.changed() || wheel_dimension(&response, &mut self.custom_width, 320)
                        {
                            self.pending_changed();
                        }
                    });
                    ui.vertical(|ui| {
                        let height_heading = ui.label(egui::RichText::new("CUSTOM HEIGHT").small().strong());
                        let response = ui
                            .add(
                                egui::TextEdit::singleline(&mut self.custom_height)
                                    .hint_text("Custom height")
                                    .desired_width(160.0),
                            ).labelled_by(height_heading.id);
                        if response.changed() || wheel_dimension(&response, &mut self.custom_height, 200)
                        {
                            self.pending_changed();
                        }
                    });
                });
            }

            ui.horizontal(|ui| {
                let availability_color = match self.availability_ok {
                    Some(true) => self.palette.accent,
                    _ => self.palette.muted,
                };
                ui.label(egui::RichText::new(&self.availability).color(availability_color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.validation).color(self.palette.warning));
                });
            });

            ui.checkbox(
                &mut self.create_backup,
                "Create a timestamped backup before applying",
            );
        }).response.rect
    }

    pub(super) fn summary_card(
        &mut self,
        ui: &mut egui::Ui,
        min_height: f32,
        preview_height: Option<f32>,
    ) -> (egui::Rect, f32) {
        let palette = self.palette;
        let response = card_frame(ui, palette).show(ui, |ui| {
            ui.set_min_height((min_height - 22.0).max(0.0));
            ui.heading("Preview");
            ui.label("Compare current and pending settings.");
            section_rule(ui, palette.accent);
            ui.label(&self.current_label);
            ui.add(
                egui::Label::new(egui::RichText::new(&self.pending_label).strong().heading())
                    .wrap(),
            );
            if let Some(comparison) = &self.external_change_notice {
                ui.label(egui::RichText::new(comparison).color(palette.warning));
            }
            if self.original.is_none() {
                ui.label(egui::RichText::new("Select a configuration to preview changes.").small());
            } else if self.pending.is_none() {
                ui.label(
                    egui::RichText::new("Enter a valid resolution to preview changes.")
                        .small()
                        .color(palette.warning),
                );
            } else if self.has_pending_changes() {
                ui.label(
                    egui::RichText::new("• Unsaved changes — review, then Apply")
                        .small()
                        .color(palette.accent),
                );
            } else {
                ui.label(
                    egui::RichText::new("✔ In sync with file")
                        .small()
                        .color(palette.muted),
                );
            }
            let used_preview_height = self.aspect_preview(ui, preview_height);
            ui.label(egui::RichText::new("LIVE ASPECT PREVIEW").small().strong());
            ui.add(
                egui::Label::new("16:9 fills the screen; narrower modes show pillarboxing.").wrap(),
            );
            ui.horizontal(|ui| {
                if ui.button("Manage backups").clicked() {
                    self.open_backups();
                }
                if ui.button("Diagnostics").clicked() {
                    self.open_diagnostics();
                }
            });
            used_preview_height
        });
        (response.response.rect, response.inner)
    }

    pub(super) fn aspect_preview(&self, ui: &mut egui::Ui, height: Option<f32>) -> f32 {
        let resolution = self.pending;
        // Reserve room for the caption and actions below the preview.
        let preview_height =
            height.unwrap_or_else(|| (ui.available_height() - 90.0).clamp(80.0, 220.0));
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(ui.available_width(), preview_height),
            egui::Sense::hover(),
        );
        let bounds = response.rect;
        let viewport = preview_viewport(bounds);
        painter.rect_filled(viewport, 2.0_f32, self.palette.preview);
        painter.rect_stroke(
            viewport,
            2.0_f32,
            egui::Stroke::new(1.0_f32, self.palette.border),
            egui::StrokeKind::Inside,
        );
        let rect = resolution
            .map(|r| preview_rectangle(viewport, r.width, r.height))
            .unwrap_or(egui::Rect::NOTHING);
        if rect.width() > 0.0 && rect.height() > 0.0 {
            let resolution = resolution.expect("preview rectangle requires a resolution");
            painter.rect_filled(rect, 1.0_f32, self.palette.preview_fill);
            painter.rect_stroke(
                rect,
                1.0_f32,
                egui::Stroke::new(2.0_f32, self.palette.accent),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{} × {}", resolution.width, resolution.height),
                egui::FontId::new(16.0, egui::FontFamily::Proportional),
                self.palette.text,
            );
        }
        preview_height
    }
}

fn card_frame(ui: &egui::Ui, palette: ThemePalette) -> egui::Frame {
    egui::Frame::group(ui.style())
        .fill(palette.card)
        .stroke(egui::Stroke::new(1.0_f32, palette.border))
        .corner_radius(egui::CornerRadius::same(palette.radius))
        .inner_margin(10.0)
}

/// Thin accent rule under a section heading; visually groups each card.
fn section_rule(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, color);
    ui.add_space(4.0);
}

fn named_combo(response: &egui::Response, name: &'static str, enabled: bool) {
    // In egui 0.32, ComboBox::from_id_salt registers an empty WidgetInfo label.
    // Labelled-by relations alone are not surfaced as a name by Windows UIA.
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, enabled, name));
}

fn wheel_dimension(response: &egui::Response, text: &mut String, minimum: usize) -> bool {
    let Ok(value) = text.parse::<usize>() else {
        return false;
    };
    if !(minimum..=32768).contains(&value) {
        return false;
    }
    if let Some(index) = wheel_selection(response, 32768 - value, 32769 - minimum) {
        *text = (32768 - index).to_string();
        true
    } else {
        false
    }
}

/// Closed selectors consume wheel input only while hovered and enabled.
/// Open menus retain their normal scrolling behavior.
pub(super) fn wheel_selection(
    response: &egui::Response,
    current: usize,
    count: usize,
) -> Option<usize> {
    if !response.enabled()
        || !response.hovered()
        || egui::Popup::is_any_open(&response.ctx)
        || count == 0
    {
        return None;
    }
    let steps = response.ctx.input_mut(|input| {
        let mut steps = 0_i32;
        input.events.retain(|event| {
            if let egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
            } = event
            {
                if delta.y != 0.0
                    && !modifiers.ctrl
                    && !modifiers.command
                    && !modifiers.alt
                    && !modifiers.shift
                {
                    let magnitude = match unit {
                        egui::MouseWheelUnit::Line => delta.y.abs().round().max(1.0),
                        _ => 1.0,
                    };
                    steps += (-delta.y.signum() * magnitude.min(100.0)) as i32;
                    return false;
                }
            }
            true
        });
        if steps != 0 {
            input.raw_scroll_delta.y = 0.0;
            input.smooth_scroll_delta.y = 0.0;
        }
        steps
    });
    let next = current.saturating_add_signed(steps as isize).min(count - 1);
    (next != current).then_some(next)
}

/// Fixed 16:9 canvas inside the available bounds (mirrors
/// `CalculatePreviewViewport`).
pub(super) fn preview_viewport(bounds: egui::Rect) -> egui::Rect {
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return egui::Rect::NOTHING;
    }
    const SCREEN_ASPECT: f32 = 16.0 / 9.0;
    let viewport_width = bounds.width().min(bounds.height() * SCREEN_ASPECT);
    let viewport_height = viewport_width / SCREEN_ASPECT;
    egui::Rect::from_min_size(
        egui::Pos2::new(
            bounds.min.x + (bounds.width() - viewport_width) / 2.0,
            bounds.min.y + (bounds.height() - viewport_height) / 2.0,
        ),
        egui::Vec2::new(viewport_width, viewport_height),
    )
}

/// Pending-resolution rectangle on a shared static scale (mirrors
/// `CalculatePreviewRectangle`): every standard family shares one height so
/// narrower aspects read as pillarboxing.
pub(super) fn preview_rectangle(bounds: egui::Rect, width: i32, height: i32) -> egui::Rect {
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 || width <= 0 || height <= 0 {
        return egui::Rect::NOTHING;
    }
    const WIDEST_STANDARD_ASPECT: f32 = 16.0 / 9.0;
    let aspect = width as f32 / height as f32;
    let reference_aspect = WIDEST_STANDARD_ASPECT.max(aspect);
    let shared_height = bounds.height().min(bounds.width() / reference_aspect);
    let preview_width = shared_height * aspect;
    egui::Rect::from_min_size(
        egui::Pos2::new(
            bounds.min.x + (bounds.width() - preview_width) / 2.0,
            bounds.min.y + (bounds.height() - shared_height) / 2.0,
        ),
        egui::Vec2::new(preview_width, shared_height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_dropdown_opens_after_pointer_click() {
        for theme in UiTheme::ALL {
            for name in [
                "Steam account",
                "Target display",
                "Resolution preset",
                "Aspect ratio",
            ] {
                check_dropdown_click(theme, name);
            }
        }
    }

    fn render_cards(
        ctx: &egui::Context,
        app: &mut ResEditApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 640.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.show_main_cards(ui);
                });
            },
        )
    }

    fn check_dropdown_click(theme: UiTheme, name: &str) {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = ResEditApp::new(None, None, None);
        app.palette = ThemePalette::for_theme(theme);
        app.palette.apply(&ctx, false);
        app.accounts
            .push(custom_account(Path::new("C:/fixture/cs2_video.txt")));
        app.accounts[0].has_config = false;
        app.accounts.push(app.accounts[0].clone());
        app.accounts[1].display_name = "Second fixture account".into();
        app.selected_account = Some(0);
        app.displays.push(DisplayInfo {
            device_name: "fixture".into(),
            friendly_name: "Fixture display".into(),
            is_primary: true,
            current_width: 1280,
            current_height: 960,
            modes: vec![],
        });
        app.display_labels.push(app.displays[0].label());
        app.displays.push(app.displays[0].clone());
        app.displays[1].device_name = "DISPLAY2".into();
        app.display_labels.push(app.displays[1].label());
        app.populate_preset_choices(0, Some(1280), Some(960));
        let mut combo_rect = None;
        for events in [vec![], vec![], vec![]] {
            let output = render_cards(&ctx, &mut app, events);
            if let Some(update) = output.platform_output.accesskit_update {
                for (_, node) in update.nodes {
                    if node.role() == egui::accesskit::Role::ComboBox && node.label() == Some(name)
                    {
                        combo_rect = node.bounds();
                    }
                }
            }
        }
        let rect = combo_rect.expect("combo bounds");
        let pos = egui::pos2(
            ((rect.x0 + rect.x1) / 2.0) as f32,
            ((rect.y0 + rect.y1) / 2.0) as f32,
        );
        for pressed in [true, false] {
            let _ = render_cards(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(
            egui::Popup::is_any_open(&ctx),
            "{theme:?}: clicking {name} must open its menu"
        );
        let mut option_rect = None;
        for _ in 0..3 {
            let output = render_cards(&ctx, &mut app, vec![]);
            if let Some(update) = output.platform_output.accesskit_update {
                for (_, node) in update.nodes {
                    if node.label() == Some("16:9") {
                        option_rect = node.bounds();
                    }
                }
            }
            assert!(
                egui::Popup::is_any_open(&ctx),
                "{theme:?}: {name} must stay open"
            );
        }
        let combo_pos = pos;
        if name != "Aspect ratio" {
            let _ = render_cards(
                &ctx,
                &mut app,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            let before = match name {
                "Steam account" => app.selected_account.unwrap(),
                "Target display" => app.selected_display,
                _ => app.selected_preset,
            };
            let _ = render_cards(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(combo_pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: egui::vec2(0.0, -1.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let after = match name {
                "Steam account" => app.selected_account.unwrap(),
                "Target display" => app.selected_display,
                _ => app.selected_preset,
            };
            assert_eq!(after, before + 1, "{theme:?}: {name} wheel selection");
            assert!(!egui::Popup::is_any_open(&ctx));
        }
        if name == "Aspect ratio" {
            let rect = option_rect.expect("16:9 menu option");
            let pos = egui::pos2(
                ((rect.x0 + rect.x1) / 2.0) as f32,
                ((rect.y0 + rect.y1) / 2.0) as f32,
            );
            for pressed in [true, false] {
                let _ = render_cards(
                    &ctx,
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(
                app.aspect_index, 1,
                "{theme:?}: menu selection must update aspect"
            );
            assert!(
                !egui::Popup::is_any_open(&ctx),
                "selection must close the popup"
            );
            for (delta, expected) in [(-1.0, 2), (-1.0, 2), (1.0, 1), (1.0, 0), (1.0, 0)] {
                let _ = render_cards(
                    &ctx,
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(combo_pos),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Line,
                            delta: egui::vec2(0.0, delta),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                assert_eq!(
                    app.aspect_index, expected,
                    "{theme:?}: bounded wheel selection"
                );
                assert!(!egui::Popup::is_any_open(&ctx), "wheel must not open menu");
            }
        }
    }

    #[test]
    fn combo_boxes_have_accesskit_names() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let names = [
            "Steam account",
            "Target display",
            "Resolution preset",
            "Aspect ratio",
        ];
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for name in names {
                    let combo = egui::ComboBox::from_id_salt(name)
                        .selected_text("Selected")
                        .show_ui(ui, |_| {});
                    named_combo(&combo.response, name, ui.is_enabled());
                }
            });
        });
        let update = output
            .platform_output
            .accesskit_update
            .expect("AccessKit tree");
        let actual: Vec<_> = update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == egui::accesskit::Role::ComboBox)
            .filter_map(|(_, node)| node.label())
            .collect();
        for name in names {
            assert!(actual.contains(&name), "missing accessible name: {name}");
        }
    }
}
