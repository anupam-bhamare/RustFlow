use slint::{ComponentHandle, Model, ToSharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::io::Read; 
slint::include_modules!();
use serde::Serialize;
use std::fs::File;
use std::io::Write;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct JsonNode {
    pub id: String,
    pub label: String,

    #[serde(rename = "node_type")]
    pub node_type: String,
    pub x_pos: f32,
    pub y_pos: f32,
    pub value: String,
    pub bg_color: String,
    pub creation_mode: i32,
 
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct JsonWire {
    pub from: String,
    pub to: String,
    layout_mode: i32,
    pub from_port: String,
    pub to_port: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FlowExport {
    pub nodes: Vec<JsonNode>,
    pub wires: Vec<JsonWire>,
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    // 1. Create a weak handle to pass into the event loop safely
    let ui_weak = ui.as_weak();
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let window = ui.window();
            window.set_fullscreen(false); 
            window.set_maximized(true);
        }
    }).unwrap();

    // =========================================
    // SYMBOL MODEL
    // =========================================
    let symbols_model = Rc::new(VecModel::<SymbolEntry>::default());
    ui.set_all_symbols(symbols_model.clone().into());

    // =========================================
    // CURRENT TOOL
    // =========================================
    let current_tool = Rc::new(RefCell::new("Text".to_string()));

    // =========================================
    // SELECT TOOL
    // =========================================
    {
        let tool_selection = current_tool.clone();
        ui.on_select_library_symbol(move |name| {
            *tool_selection.borrow_mut() = name.to_string();
        });
    }

    // =========================================
    // PLACE SYMBOL (Updated to capture node_type)
    // =========================================
    {
        let place_model = symbols_model.clone();
        let place_tool = current_tool.clone();
        let ui_weak = ui.as_weak();
ui.on_place_symbol(move |name, node_type, bg_color, x, y,mode| {
            place_model.push(SymbolEntry {
                name: name.clone(),
                label: name.clone(),
                node_type: node_type,     // Maps cleanly from argument #2
                bg_color: bg_color,       // Maps cleanly from argument #3
                x,                        // Maps cleanly from argument #4
                y,                        // Maps cleanly from argument #5
                value: "".into(),
                creation_mode:mode
            });
        });
    }

    // =========================================
    // MOVE SYMBOL
    // =========================================
    {
        let move_model = symbols_model.clone();
        ui.on_update_symbol_position(move |index, x, y| {
            if let Some(mut symbol) = move_model.row_data(index as usize) {
                symbol.x = x;
                symbol.y = y;
                move_model.set_row_data(index as usize, symbol);
            }
        });
    }

    // =========================================
    // DELETE SELECTED NODE
    // =========================================
    {
        let delete_model = symbols_model.clone();
        let ui_weak = ui.as_weak();

        ui.on_delete_selected_symbol(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let index = ui.get_selected_index();
                if index != -1 {
                    delete_model.remove(index as usize);
                    ui.set_selected_index(-1);
                }
            }
        });
    }

    // =========================================
    // CLEAR CANVAS
    // =========================================
    {
        let clear_model = symbols_model.clone();
        ui.on_clear_canvas(move || {
            clear_model.set_vec(vec![]);
        });
    }

    // =========================================
    // IMPORT XML
    // =========================================
    {
        let import_model = symbols_model.clone();
        ui.on_import_xml(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("XML Files", &["xml"])
                .pick_file()
            {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(doc) = roxmltree::Document::parse(&content) {
                        import_model.set_vec(vec![]);
                        for node in doc.descendants().filter(|n| n.has_tag_name("symbol")) {
                            let name = node.attribute("name").unwrap_or("Text").to_string();
                            let x = node.attribute("x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                            let y = node.attribute("y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                            let value = node.attribute("value").unwrap_or("").to_string();

                            import_model.push(SymbolEntry {
                                name: name.clone().into(),
                                label: name.into(),
                                node_type: "ui_text".into(),
                                x,
                                y,
                                value: value.into(),
                                bg_color: "#0d6efd".into(),
                                creation_mode:0
                            });
                        }
                    }
                }
            }
        });
    }

    // =========================================
    // CONNECTION MODEL
    // =========================================
    let connections = Rc::new(VecModel::from(Vec::<Connection>::new()));
    ui.set_connections(connections.clone().into());

    // =========================================
    // WIRING LOGIC
    // =========================================
   let wire_source = Rc::new(Cell::new(-1));
let active_source_port = Rc::new(RefCell::new(String::new()));

ui.on_handle_port_click({
    let conn_model = connections.clone();
    let wire_source = wire_source.clone();
    let src_port = active_source_port.clone();

    move |node_index, is_input, mode, port_name| {
        let port_str = port_name.to_string();
        if !is_input {
            wire_source.set(node_index);
            *src_port.borrow_mut() = port_str;
        } else {
            let source = wire_source.get();
            if source != -1 && source != node_index {
                println!("Selected target node: {}", node_index);
                
                // FIX: Dereference the borrow guard and take the underlying String value cleanly
                let saved_port = std::mem::take(&mut *src_port.borrow_mut());
                let from_port_shared = slint::SharedString::from(saved_port);
                let to_port_shared = slint::SharedString::from(port_str);

                conn_model.push(Connection {
                    from_index: source,
                    to_index: node_index,
                    selected: false,
                    creation_mode: mode,
                    from_port: from_port_shared,
                    to_port: to_port_shared,
                });
                println!("Connected {} -> {}", source, node_index);
            }
            wire_source.set(-1);
        }
    }
});
    // =========================================
    // SELECT CONNECTION
    // =========================================
    ui.on_select_connection({
        let connections = connections.clone();
        move |index| {
            for i in 0..connections.row_count() {
                if let Some(mut conn) = connections.row_data(i) {
                    conn.selected = i == index as usize;
                    connections.set_row_data(i, conn);
                }
            }
            println!("Selected connection {}", index);
        }
    });

    // =========================================
    // DELETE CONNECTION
    // =========================================
    ui.on_delete_selected_connection({
        let connections = connections.clone();
        move || {
            let mut delete_index = None;
            for i in 0..connections.row_count() {
                if let Some(conn) = connections.row_data(i) {
                    if conn.selected {
                        delete_index = Some(i);
                        break;
                    }
                }
            }
            if let Some(index) = delete_index {
                connections.remove(index);
                println!("Deleted connection {}", index);
            }
        }
    });

    // =========================================
    // SAVE PROPERTIES
    // =========================================
    ui.on_save_node_properties({
        let model = symbols_model.clone();
        move |index, label, value| {
            if let Some(mut node) = model.row_data(index as usize) {
                node.label = label;
                node.value = value;
                model.set_row_data(index as usize, node);
            }
        }
    });

    // =========================================
    // SAVE FLOW (Scoping & Type Inference Fixed)
    // =========================================
    {
        // 1. Create a fresh local weak pointer that hasn't been moved by another closure
        let ui_save_weak = ui.as_weak();

        let export_symbols_active = symbols_model.clone();
        let export_connections_active = connections.clone();

        ui.on_save_flow(move || {
            let mut json_nodes = Vec::new();

            // 2. Map Slint Nodes from your active shared Vector Model
            for i in 0..export_symbols_active.row_count() {
                if let Some(item) = export_symbols_active.row_data(i) {
                    let explicit_id = (i + 1).to_string();

                    json_nodes.push(JsonNode {
                        id: explicit_id, 
                        label: if item.label.is_empty() { item.name.to_string() } else { item.label.to_string() },
                        node_type: item.node_type.to_string(), 
                        x_pos: item.x,
                        y_pos: item.y,
                        value: item.value.to_string(),
                        bg_color: if item.bg_color.is_empty() { "#0d6efd".to_string() } else { item.bg_color.to_string() },
                        creation_mode: item.creation_mode,

                    });
                }
            }

            // 3. Map Slint Connections into wires
            let mut json_wires = Vec::new();
            for i in 0..export_connections_active.row_count() {
                if let Some(conn) = export_connections_active.row_data(i) {
                    json_wires.push(JsonWire {
                        from: format!("node_{}", conn.from_index + 1),
                        to: format!("node_{}", conn.to_index + 1),
                        layout_mode: conn.creation_mode,
                        from_port:conn.from_port.to_string(),
                        to_port:conn.to_port.to_string()
                    });
                }
            }

            // 4. Assemble the full manifest bundle
            let export_data = FlowExport {
                nodes: json_nodes,
                wires: json_wires,
            };

            // 5. Generate dynamic filename using current timestamp
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
            let filename = format!("flow_layout_{}.json", timestamp);

            // 6. Save file out to disk and trigger popup dialog alerts via upgraded weak reference
            if let Ok(json_string) = serde_json::to_string_pretty(&export_data) {
                match File::create(&filename) {
                    Ok(mut file) => {
                        let _ = file.write_all(json_string.as_bytes());
                        
                        // Safely upgrade our fresh, local weak handle
                        if let Some(ui_active) = ui_save_weak.upgrade() {
                            let message = slint::SharedString::from(format!(
                                "Flow configuration successfully saved to:\n{}", 
                                filename
                            ));
                            ui_active.invoke_trigger_alert(message);
                        }
                    }
                    Err(e) => {
                        if let Some(ui_active) = ui_save_weak.upgrade() {
                            let message = slint::SharedString::from(format!(
                                "Failed to create file:\n{:?}", 
                                e
                            ));
                            ui_active.invoke_trigger_alert(message);
                        }
                    }
                }
            }
        });
    }
    
    // =========================================
    // LOAD FLOW (Model Methods & Struct Fields Fixed)
    // =========================================
    {
        let import_symbols = symbols_model.clone();
        let import_connections = connections.clone();
        let ui_load_weak = ui.as_weak();

        ui.on_load_flow(move || {
            // 1. Open Native File Picker Filtered for JSON
            let file_picker = rfd::FileDialog::new()
                .add_filter("JSON Flow Profiles", &["json"])
                .set_title("Select Flow Configuration Layout")
                .pick_file();

            let path = match file_picker {
                Some(p) => p,
                None => return,
            };

            // 2. Read string payload contents from selected file
            let file_data = match File::open(&path) {
                Ok(mut file) => {
                    let mut contents = String::new();
                    if file.read_to_string(&mut contents).is_ok() {
                        contents
                    } else {
                        return;
                    }
                }
                Err(_) => return,
            };

            // 3. Deserialize JSON configuration back into Memory
            let decoded_flow: FlowExport = match serde_json::from_str(&file_data) {
                Ok(flow) => flow,
                Err(e) => {
                    if let Some(ui_active) = ui_load_weak.upgrade() {
                        let msg = slint::SharedString::from(format!("Failed to parse layout configuration file:\n{:?}", e));
                        ui_active.invoke_trigger_alert(msg);
                    }
                    return;
                }
            };

            // 4. Create local vectors to hold the loaded records
            let mut fresh_symbols = Vec::new();
            let mut fresh_connections = Vec::new();

            // 5. Populate Symbols/Nodes
            for node in decoded_flow.nodes {
                fresh_symbols.push(SymbolEntry {
                    name: node.label.clone().into(),
                    label: node.label.into(),
                    node_type: node.node_type.into(),
                    x: node.x_pos,
                    y: node.y_pos,
                    value: node.value.into(),
                    bg_color: node.bg_color.into(),
                    creation_mode: node.creation_mode,
                });
            }

            // 6. Map Wires (Converting 1-indexed string representations back to numbers)
            for wire in decoded_flow.wires {
                let from_id_str = wire.from.replace("node_", "");
                let to_id_str = wire.to.replace("node_", "");

                if let (Ok(from_val), Ok(to_val)) = (from_id_str.parse::<i32>(), to_id_str.parse::<i32>()) {
                    fresh_connections.push(Connection {
                        from_index: from_val - 1,
                        to_index: to_val - 1,
                        selected: false, 
                        creation_mode:wire.layout_mode,// <-- FIXED: Added the missing field initialization
                // Convert native Rust String types directly into Slint's SharedString structure
            from_port: slint::SharedString::from(&wire.from_port),
            to_port: slint::SharedString::from(&wire.to_port),
                    });
                }
            }

            // 7. SWAP CORES VIA .set_vec() - Cleans canvas and applies fresh layouts cleanly
            import_symbols.set_vec(fresh_symbols);
            import_connections.set_vec(fresh_connections);

            // 8. Fire Success Alert
            if let Some(ui_active) = ui_load_weak.upgrade() {
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                let message = slint::SharedString::from(format!(
                    "Flow composition parsed perfectly!\nLoaded File: {}", 
                    filename
                ));
                ui_active.invoke_trigger_alert(message);
            }
        });
    }   ui.run()
}