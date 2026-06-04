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
    pub layout_mode: i32,
    pub from_port: String,
    pub to_port: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FlowExport {
    pub nodes: Vec<JsonNode>,
    pub wires: Vec<JsonWire>,
}

// History stack snapshot structure for managing Undo functionality
#[derive(Clone, Debug)]
pub struct CanvasStateSnapshot {
    pub symbols: Vec<SymbolEntry>,
    pub connections: Vec<Connection>,
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    // Create a weak reference to safely pass into the event loop closure
    let weak_app = ui.as_weak();
    
    // Schedule the window to maximize once the event loop starts spin-up
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = weak_app.upgrade() {
            ui.window().set_maximized(true);
        }
    }).unwrap();

    // =========================================
    // DATA MODEL INITIALIZATIONS
    // =========================================
    let symbols_model = Rc::new(VecModel::<SymbolEntry>::default());
    ui.set_all_symbols(symbols_model.clone().into());

    let connections = Rc::new(VecModel::from(Vec::<Connection>::new()));
    ui.set_connections(connections.clone().into());

    let current_tool = Rc::new(RefCell::new("Text".to_string()));
    
    // Initialize the historical state snapshot stack
    let history_stack = Rc::new(RefCell::new(Vec::<CanvasStateSnapshot>::new()));
    let redo_stack = Rc::new(RefCell::new(Vec::<CanvasStateSnapshot>::new())); // Your initialization

    // =========================================
    // CORE HISTORY CAPTURE ENGINE
    // =========================================
    let save_history: Rc<dyn Fn()> = {
        let s_model = symbols_model.clone();
        let c_model = connections.clone();
        let stack = history_stack.clone();
        let r_stack = redo_stack.clone(); // <--- CLONE IT HERE FIRST
        
        Rc::new(move || {
            let current_symbols: Vec<SymbolEntry> = s_model.iter().collect();
            let current_conns: Vec<Connection> = c_model.iter().collect();
            
            stack.borrow_mut().push(CanvasStateSnapshot {
                symbols: current_symbols,
                connections: current_conns,
            });
            
            // Clear the future timeline because a new action occurred
            r_stack.borrow_mut().clear(); // <--- Uses the cloned pointer safely!
            
            if stack.borrow().len() > 50 {
                stack.borrow_mut().remove(0);
            }
        })
    };
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
    // PLACE SYMBOL
    // =========================================
    {
        let place_model = symbols_model.clone();
        let save_h = save_history.clone();
        
        ui.on_place_symbol(move |name, node_type, bg_color, x, y, mode| {
            save_h(); // Log history block state before structural push
            place_model.push(SymbolEntry {
                name: name.clone(),
                label: name.clone(),
                node_type,     
                bg_color,      
                x,                                
                y,                                
                value: "".into(),
                creation_mode: mode,
                //is_selected: false,
            });
        });
    }

    // =========================================
    // MOVE SYMBOL
    // =========================================
    {
        let move_model = symbols_model.clone();
        let save_h = save_history.clone();
        
        ui.on_update_symbol_position(move |index, x, y| {
            if let Some(mut symbol) = move_model.row_data(index as usize) {
                save_h(); // Capture state layout snapshot before moving
                symbol.x = x;
                symbol.y = y;
                move_model.set_row_data(index as usize, symbol);
            }
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
                                creation_mode: 0,
                                //is_selected: false,
                            });
                        }
                    }
                }
            }
        });
    }

    // =========================================
    // WIRING LOGIC
    // =========================================
    let wire_source = Rc::new(Cell::new(-1));
    let active_source_port = Rc::new(RefCell::new(String::new()));

    ui.on_handle_port_click({
        let conn_model = connections.clone();
        let wire_source = wire_source.clone();
        let src_port = active_source_port.clone();
        let save_h = save_history.clone();

        move |node_index, _is_input, mode, port_name| {
            let port_str = port_name.to_string();
            let source = wire_source.get();
            
            if source == -1 {
                wire_source.set(node_index);
                *src_port.borrow_mut() = port_str;
                println!("Drag STARTED from Node {}, Port {}", node_index, port_name);
            } else {
                if source != node_index {
                    let first_port = src_port.borrow().clone();
                    let second_port = port_str.clone();

                    let mut final_from_index = source;
                    let mut final_to_index = node_index;
                    let mut from_port_shared = slint::SharedString::from(first_port.clone());
                    let mut to_port_shared = slint::SharedString::from(second_port.clone());

                    if first_port == "left" {
                        final_from_index = node_index; 
                        final_to_index = source;       
                        from_port_shared = slint::SharedString::from(second_port.clone());
                        to_port_shared = slint::SharedString::from(first_port.clone());
                    }

                    save_h(); // Log the system layout topology before pushing connection wire

                    conn_model.push(Connection {
                        from_index: final_from_index,
                        to_index: final_to_index,
                        selected: false,
                        creation_mode: mode,
                        from_port: from_port_shared.clone(),
                        to_port: to_port_shared.clone(),
                    });
                    
                    println!("Line drawn from Node {}/{} to Node {}/{}", 
                        final_from_index, from_port_shared, final_to_index, to_port_shared);            
                }
                
                wire_source.set(-1);
                *src_port.borrow_mut() = String::new();
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
        let save_h = save_history.clone();
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
                save_h(); // Save snapshot prior to wire deletion
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
        let save_h = save_history.clone();
        move |index, label, value| {
            if let Some(mut node) = model.row_data(index as usize) {
                save_h(); // Snapshot property states before text alteration updates
                node.label = label;
                node.value = value;
                model.set_row_data(index as usize, node);
            }
        }
    });

    // =========================================
    // SAVE FLOW (JSON Export Handler)
    // =========================================
    {
        let ui_save_weak = ui.as_weak();
        let export_symbols_active = symbols_model.clone();
        let export_connections_active = connections.clone();

        ui.on_save_flow(move || {
            if export_symbols_active.row_count() == 0 {
                if let Some(ui_active) = ui_save_weak.upgrade() {
                    let message = slint::SharedString::from("There is no flow configuration to save.");
                    ui_active.invoke_trigger_alert(message);
                }
                return; 
            }
            let mut json_nodes = Vec::new();

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

            let mut json_wires = Vec::new();
            for i in 0..export_connections_active.row_count() {
                if let Some(conn) = export_connections_active.row_data(i) {
                    json_wires.push(JsonWire {
                        from: format!("node_{}", conn.from_index + 1),
                        to: format!("node_{}", conn.to_index + 1),
                        layout_mode: conn.creation_mode,
                        from_port: conn.from_port.to_string(),
                        to_port: conn.to_port.to_string()
                    });
                }
            }

            let export_data = FlowExport {
                nodes: json_nodes,
                wires: json_wires,
            };

            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
            let filename = format!("flow_layout_{}.json", timestamp);

            if let Ok(json_string) = serde_json::to_string_pretty(&export_data) {
                match File::create(&filename) {
                    Ok(mut file) => {
                        let _ = file.write_all(json_string.as_bytes());
                        if let Some(ui_active) = ui_save_weak.upgrade() {
                            let message = slint::SharedString::from(format!("Flow configuration successfully saved to:\n{}", filename));
                            ui_active.invoke_trigger_alert(message);
                        }
                    }
                    Err(e) => {
                        if let Some(ui_active) = ui_save_weak.upgrade() {
                            let message = slint::SharedString::from(format!("Failed to create file:\n{:?}", e));
                            ui_active.invoke_trigger_alert(message);
                        }
                    }
                }
            }
        });
    }
    
    // =========================================
    // LOAD FLOW (JSON De-serialization Engine)
    // =========================================
    {
        let import_symbols = symbols_model.clone();
        let import_connections = connections.clone();
        let ui_load_weak = ui.as_weak();
        let save_h = save_history.clone();

        ui.on_load_flow(move || {
            let file_picker = rfd::FileDialog::new()
                .add_filter("JSON Flow Profiles", &["json"])
                .set_title("Select Flow Configuration Layout")
                .pick_file();

            let path = match file_picker {
                Some(p) => p,
                None => return,
            };

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

            save_h(); // Log history snapshot so a freshly wiped canvas step can be undone

            let mut fresh_symbols = Vec::new();
            let mut fresh_connections = Vec::new();

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
                    //is_selected: false,
                });
            }

            for wire in decoded_flow.wires {
                let from_id_str = wire.from.replace("node_", "");
                let to_id_str = wire.to.replace("node_", "");

                if let (Ok(from_val), Ok(to_val)) = (from_id_str.parse::<i32>(), to_id_str.parse::<i32>()) {
                    fresh_connections.push(Connection {
                        from_index: from_val - 1,
                        to_index: to_val - 1,
                        selected: false, 
                        creation_mode: wire.layout_mode,
                        from_port: slint::SharedString::from(&wire.from_port),
                        to_port: slint::SharedString::from(&wire.to_port),
                    });
                }
            }

            import_symbols.set_vec(fresh_symbols);
            import_connections.set_vec(fresh_connections);

            if let Some(ui_active) = ui_load_weak.upgrade() {
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                let message = slint::SharedString::from(format!("Flow composition parsed successfully!\nLoaded File: {}", filename));
                ui_active.invoke_trigger_alert(message);
            }
        });
    } 

    // =========================================
    // DELETE SYMBOL
    // =========================================
    {
        let delete_model = symbols_model.clone();
        let ui_weak = ui.as_weak();      
        let save_h = save_history.clone();

        ui.on_delete_selected_symbol({
            let connections = connections.clone();
            let delete_model = delete_model.clone();
            let save_h = save_h.clone();

            move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let index = ui.get_selected_index();
                    if index != -1 {
                        save_h(); // Log step context state map data changes before removing indices

                        let target_index = index as i32;
                        for i in (0..connections.row_count()).rev() {
                            if let Some(mut conn) = connections.row_data(i) {
                                if conn.from_index == target_index || conn.to_index == target_index {
                                    connections.remove(i);
                                    println!("Removed broken connection at index {}", i);
                                } else {
                                    let mut changed = false;
                                    if conn.from_index > target_index {
                                        conn.from_index -= 1;
                                        changed = true;
                                    }
                                    if conn.to_index > target_index {
                                        conn.to_index -= 1;
                                        changed = true;
                                    }
                                    if changed {
                                        connections.set_row_data(i, conn);
                                    }
                                }
                            }
                        }

                        let index_usize = index as usize;
                        if index_usize < delete_model.row_count() {
                            delete_model.remove(index_usize);
                            println!("Deleted symbol at index {}", index_usize);
                        }

                        ui.set_selected_index(-1);
                    }
                }
            }
        });
    }

    // =========================================
    // CLEAR CANVAS
    // =========================================
    {
        let clear_model = symbols_model.clone();
        let clear_connections = connections.clone();
        let save_h = save_history.clone();
        ui.on_clear_canvas(move || {
            save_h(); // Capture history framework stack frame state before total wipeouts
            clear_model.set_vec(vec![]);
            clear_connections.set_vec(vec![]);
        });
    }


   // =========================================
    // UNDO ENGINE ACTION HANDLER (Updated for Redo)
    // =========================================
    {
        let undo_symbols = symbols_model.clone();
        let undo_connections = connections.clone();
        let undo_stack = history_stack.clone();
        let r_stack = redo_stack.clone(); // Kept variable name clean
        
        ui.on_undo_action(move || {
            let mut u_stack = undo_stack.borrow_mut();
            let mut redo_stk = r_stack.borrow_mut();
            
            // 1. Pop the previous state off the undo stack
            if let Some(previous_state) = u_stack.pop() {
                
                // ─── THE REDO MAGIC HAPPENS HERE ───
                // 2. Capture what the canvas looks like RIGHT NOW (Before restoring the past)
                let current_state = CanvasStateSnapshot {
                    symbols: undo_symbols.iter().collect(),
                    connections: undo_connections.iter().collect(),
                };
                
                // 3. Push that current state onto the redo stack
                redo_stk.push(current_state);
                
                // 4. Finally, travel back in time by restoring the old state
                undo_symbols.set_vec(previous_state.symbols);
                undo_connections.set_vec(previous_state.connections);
                
                println!("Undo executed. Undo stack: {}, Redo stack: {}", u_stack.len(), redo_stk.len());
            } else {
                println!("No historical snapshot items left to restore.");
            }
        });
    }
    // =========================================
    // REDO ENGINE ACTION HANDLER
    // =========================================
    {
        let redo_symbols = symbols_model.clone();
        let redo_connections = connections.clone();
        let undo_stack = history_stack.clone();
        let r_stack = redo_stack.clone();
        
        ui.on_redo_action(move || {
            let mut u_stack = undo_stack.borrow_mut();
            let mut redo_stk = r_stack.borrow_mut();
            
            // 1. Check if there is a "future" state available to restore
            if let Some(next_state) = redo_stk.pop() {
                
                // 2. Capture the current state of the canvas before overwriting it
                let current_state = CanvasStateSnapshot {
                    symbols: redo_symbols.iter().collect(),
                    connections: redo_connections.iter().collect(),
                };
                
                // 3. Push the current state onto the undo stack so the user can "Undo" this Redo
                u_stack.push(current_state);
                
                // 4. Apply the popped state back to the active canvas models
                redo_symbols.set_vec(next_state.symbols);
                redo_connections.set_vec(next_state.connections);
                
                println!("Redo executed. Undo stack: {}, Redo stack: {}", u_stack.len(), redo_stk.len());
            } else {
                println!("No actions left to redo!");
            }
        });
    }
    ui.run()
}