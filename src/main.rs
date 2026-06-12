use slint::{ComponentHandle, Model, ModelRc, ToSharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;

slint::include_modules!();

#[derive(Clone, Debug)]
struct SymbolClipboardData {
    name: String,
    label: String,
    value: String,
    x: f32,
    y: f32,
    node_type: String,
    bg_color: String,
    creation_mode: i32,
}

#[derive(Clone, Debug)]
struct WireClipboardData {
    from_index: i32,
    to_index: i32,
    creation_mode: i32,
    from_port: String,
    to_port: String,
}

#[derive(Clone, Debug)]
struct SubgraphClipboard {
    nodes: Vec<(usize, SymbolClipboardData)>,
    wires: Vec<WireClipboardData>,
}

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

#[derive(Clone, Debug)]
pub struct CanvasStateSnapshot {
    pub symbols: Vec<SymbolEntry>,
    pub connections: Vec<Connection>,
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let weak_app = ui.as_weak();
    let symbols_model = Rc::new(VecModel::<SymbolEntry>::default());
    ui.set_all_symbols(symbols_model.clone().into());
    let connections = Rc::new(VecModel::<Connection>::default());
    ui.set_connections(connections.clone().into());
    let current_tool = Rc::new(RefCell::new("Text".to_string()));
    let subgraph_clipboard: Rc<RefCell<Option<SubgraphClipboard>>> = Rc::new(RefCell::new(None));
    let ui_weak_select = ui.as_weak();

    slint::invoke_from_event_loop(move || {
        if let Some(ui) = weak_app.upgrade() {
            ui.window().set_maximized(true);
        }
    })
    .unwrap();


    ui.on_node_clicked(move |clicked_index, shift_pressed| {
        if let Some(ui_active) = ui_weak_select.upgrade() {
            let current_selection = ui_active.get_selected_indexes();
            let mut selection_vec: Vec<i32> = current_selection.iter().collect();

            if shift_pressed {
                if let Some(pos) = selection_vec.iter().position(|&x| x == clicked_index) {
                    selection_vec.remove(pos);
                } else {
                    selection_vec.push(clicked_index);
                }
            } else {
                selection_vec = vec![clicked_index];
            }

            println!("Selected indexes: {:?}", selection_vec);

            let slint_model: ModelRc<i32> = Rc::new(VecModel::from(selection_vec.clone())).into();
            ui_active.set_selected_indexes(slint_model);

            let all_symbols = ui_active.get_all_symbols();
            for i in 0..all_symbols.row_count() {
                if let Some(mut symbol) = all_symbols.row_data(i) {
                    let is_currently_selected = selection_vec.contains(&(i as i32));

                    if symbol.is_selected != is_currently_selected {
                        symbol.is_selected = is_currently_selected;
                        all_symbols.set_row_data(i, symbol);
                    }
                }
            }
        }
    });

    let clipboard_copy = subgraph_clipboard.clone();
    let symbols_copy_ref = symbols_model.clone();
    let wires_copy_ref = connections.clone();
    let ui_weak_copy = ui.as_weak();

    ui.on_copy_selected_subgraph(move |indexes_model| {
        let selected_indices: Vec<usize> = indexes_model.iter().map(|i| i as usize).collect();
        if selected_indices.is_empty() {
            return;
        }

        let mut copied_nodes = Vec::new();
        let mut copied_node_indices = std::collections::HashSet::new();

        for &idx in &selected_indices {
            if let Some(symbol) = symbols_copy_ref.row_data(idx) {
                copied_node_indices.insert(idx as i32);
                copied_nodes.push((
                    idx,
                    SymbolClipboardData {
                        name: symbol.name.to_string(),
                        label: symbol.label.to_string(),
                        value: symbol.value.to_string(),
                        x: symbol.x,
                        y: symbol.y,
                        node_type: symbol.node_type.to_string(),
                        bg_color: symbol.bg_color.to_string(),
                        creation_mode: symbol.creation_mode,
                    },
                ));
            }
        }

        let mut copied_wires = Vec::new();
        for i in 0..wires_copy_ref.row_count() {
            if let Some(wire) = wires_copy_ref.row_data(i) {
                if copied_node_indices.contains(&wire.from_index)
                    && copied_node_indices.contains(&wire.to_index)
                {
                    copied_wires.push(WireClipboardData {
                        from_index: wire.from_index,
                        to_index: wire.to_index,
                        creation_mode: wire.creation_mode,
                        from_port: wire.from_port.to_string(),
                        to_port: wire.to_port.to_string(),
                    });
                }
            }
        }

        let total_nodes = copied_nodes.len();
        let total_wires = copied_wires.len();

        *clipboard_copy.borrow_mut() = Some(SubgraphClipboard {
            nodes: copied_nodes,
            wires: copied_wires,
        });

        if let Some(ui_active) = ui_weak_copy.upgrade() {
            let msg = format!("Copied {} nodes.", total_nodes);
            ui_active.invoke_trigger_alert(msg.into());
        }
        println!("Successfully captured cluster to clipboard buffer.");
        let _ = std::io::stdout().flush();
    });

    let clipboard_cut = subgraph_clipboard.clone();
    let symbols_cut_ref = symbols_model.clone();
    let wires_cut_ref = connections.clone();
    let ui_weak_cut = ui.as_weak();

    ui.on_cut_selected_subgraph(move |indexes_model| {
        let mut selected_indices: Vec<usize> = indexes_model.iter().map(|i| i as usize).collect();
        if selected_indices.is_empty() {
            return;
        }

        selected_indices.sort_by(|a, b| b.cmp(a));
        selected_indices.dedup();

        let mut copied_nodes = Vec::new();
        let mut cut_node_indices = std::collections::HashSet::new();

        for (relative_id, &idx) in selected_indices.iter().rev().enumerate() {
            if let Some(symbol) = symbols_cut_ref.row_data(idx) {
                cut_node_indices.insert(idx as i32);

                copied_nodes.push((
                    relative_id,
                    SymbolClipboardData {
                        name: symbol.name.to_string(),
                        label: symbol.label.to_string(),
                        value: symbol.value.to_string(),
                        x: symbol.x,
                        y: symbol.y,
                        node_type: symbol.node_type.to_string(),
                        bg_color: symbol.bg_color.to_string(),
                        creation_mode: symbol.creation_mode,
                    },
                ));
            }
        }

        let mut copied_wires = Vec::new();
        let mut wire_indices_to_remove = Vec::new();

        for i in 0..wires_cut_ref.row_count() {
            if let Some(wire) = wires_cut_ref.row_data(i) {
                let connects_from_cut = cut_node_indices.contains(&wire.from_index);
                let connects_to_cut = cut_node_indices.contains(&wire.to_index);

                if connects_from_cut || connects_to_cut {
                    wire_indices_to_remove.push(i);

                    if connects_from_cut && connects_to_cut {
                        let rel_from = selected_indices
                            .iter()
                            .rev()
                            .position(|&x| x == wire.from_index as usize)
                            .unwrap_or(0);
                        let rel_to = selected_indices
                            .iter()
                            .rev()
                            .position(|&x| x == wire.to_index as usize)
                            .unwrap_or(0);

                        copied_wires.push(WireClipboardData {
                            from_index: rel_from as i32, // Remapped relative pin offset
                            to_index: rel_to as i32,     // Remapped relative pin offset
                            creation_mode: wire.creation_mode,
                            from_port: wire.from_port.to_string(),
                            to_port: wire.to_port.to_string(),
                        });
                    }
                }
            }
        }

        let total_nodes_cut = copied_nodes.len();

        *clipboard_cut.borrow_mut() = Some(SubgraphClipboard {
            nodes: copied_nodes,
            wires: copied_wires,
        });

        wire_indices_to_remove.sort_by(|a, b| b.cmp(a));
        wire_indices_to_remove.dedup();
        for idx in wire_indices_to_remove {
            wires_cut_ref.remove(idx);
        }

        for idx in selected_indices {
            if idx < symbols_cut_ref.row_count() {
                symbols_cut_ref.remove(idx);
            }
        }

        if let Some(ui_active) = ui_weak_cut.upgrade() {
            ui_active
                .set_selected_indexes(std::rc::Rc::new(slint::VecModel::<i32>::default()).into());
            ui_active.set_selected_index(-1);

            let msg = format!("Cut {} nodes.", total_nodes_cut);
            ui_active.invoke_trigger_alert(msg.into());
        }

        println!("Graph cluster cut operation completed successfully.");
        let _ = std::io::stdout().flush();
    });
    let clipboard_paste = subgraph_clipboard.clone();
    let ui_weak_paste = ui.as_weak();

    ui.on_paste_selected_subgraph(move |_, _| {
        if let Some(ref clipboard) = *clipboard_paste.borrow() {
            if clipboard.nodes.is_empty() {
                return;
            }

            if let Some(ui_active) = ui_weak_paste.upgrade() {
                let mut temporary_ghosts = Vec::new();
                let mut temporary_ghost_wires = Vec::new();

                let min_x = clipboard
                    .nodes
                    .iter()
                    .map(|(_, n)| n.x)
                    .fold(f32::INFINITY, f32::min);
                let min_y = clipboard
                    .nodes
                    .iter()
                    .map(|(_, n)| n.y)
                    .fold(f32::INFINITY, f32::min);

                let mut local_index_map = std::collections::HashMap::new();

                for (local_idx, (old_global_idx, node)) in clipboard.nodes.iter().enumerate() {
                    local_index_map.insert(*old_global_idx as i32, local_idx as i32);

                    let mut ghost_node = SymbolEntry::default();
                    ghost_node.label = node.label.to_shared_string();
                    ghost_node.node_type = node.node_type.to_shared_string();
                    ghost_node.bg_color = node.bg_color.to_shared_string();
                    ghost_node.x = node.x - min_x;
                    ghost_node.y = node.y - min_y;
                    ghost_node.creation_mode = node.creation_mode;

                    temporary_ghosts.push(ghost_node);
                }

                for wire in &clipboard.wires {
                    if let (Some(local_from), Some(local_to)) = (
                        local_index_map.get(&wire.from_index),
                        local_index_map.get(&wire.to_index),
                    ) {
                        let mut ghost_wire = Connection::default();
                        ghost_wire.from_index = *local_from;
                        ghost_wire.to_index = *local_to;
                        ghost_wire.from_port = wire.from_port.to_shared_string();
                        ghost_wire.to_port = wire.to_port.to_shared_string();
                        ghost_wire.creation_mode = wire.creation_mode;

                        temporary_ghost_wires.push(ghost_wire);
                    }
                }

                ui_active.set_ghost_symbols(slint::ModelRc::new(slint::VecModel::from(
                    temporary_ghosts,
                )));
                ui_active.set_ghost_connections(slint::ModelRc::new(slint::VecModel::from(
                    temporary_ghost_wires,
                )));

                ui_active.set_is_pasting_mode(true);
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_select_all(move || {
        if let Some(ui_active) = ui_weak.upgrade() {
            let all_symbols_model = ui_active.get_all_symbols();
            let total_symbols = all_symbols_model.row_count();
            println!(
                "--- Selecting All Canvas Symbols (Total: {}) ---",
                total_symbols
            );

            if let Some(vec_model) = all_symbols_model
                .as_any()
                .downcast_ref::<slint::VecModel<SymbolEntry>>()
            {
                for i in 0..total_symbols {
                    if let Some(mut symbol) = vec_model.row_data(i) {
                        symbol.is_selected = true; // Set the border flag

                        vec_model.set_row_data(i, symbol);
                    }
                }
            } else {
            }

            let all_indexes: Vec<i32> = (0..total_symbols as i32).collect();
            let slint_model: ModelRc<i32> = Rc::new(VecModel::from(all_indexes)).into();

            ui_active.set_selected_indexes(slint_model);

            if total_symbols > 0 {
                ui_active.set_selected_index((total_symbols - 1) as i32);
                let msg = format!("Selected {} nodes.", total_symbols);
                ui_active.invoke_trigger_alert(msg.into());
            }
        }
    });

    let ui_weak_drop = ui.as_weak();
    let symbols_drop_ref = symbols_model.clone();
    let wires_drop_ref = connections.clone();

    ui.on_canvas_clicked(move || {
        let ui = match ui_weak_drop.upgrade() {
            Some(v) => v,
            None => return,
        };

        if !ui.get_is_pasting_mode() {
            return;
        }

        let ghost_symbols = ui.get_ghost_symbols();
        let ghost_connections = ui.get_ghost_connections();

        let drop_x = ui.get_live_mouse_x();
        let drop_y = ui.get_live_mouse_y();

        let base_symbols_count = symbols_drop_ref.row_count() as i32;

        if let Some(vec_nodes_model) = symbols_drop_ref
            .as_any()
            .downcast_ref::<slint::VecModel<SymbolEntry>>()
        {
            for i in 0..ghost_symbols.row_count() {
                if let Some(ghost) = ghost_symbols.row_data(i) {
                    let mut real_node = SymbolEntry::default();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();

                    real_node.name = format!("node_{}_{}", i, nanos).to_shared_string();
                    real_node.label = ghost.label;
                    real_node.node_type = ghost.node_type;
                    real_node.bg_color = ghost.bg_color;
                    real_node.x = drop_x + ghost.x;
                    real_node.y = drop_y + ghost.y;
                    real_node.is_selected = true;
                    real_node.creation_mode = ghost.creation_mode;

                    vec_nodes_model.push(real_node);
                }
            }
        }

        if let Some(vec_wires_model) = wires_drop_ref
            .as_any()
            .downcast_ref::<slint::VecModel<Connection>>()
        {
            for i in 0..ghost_connections.row_count() {
                if let Some(ghost_wire) = ghost_connections.row_data(i) {
                    let mut real_wire = Connection::default();

                    real_wire.from_index = base_symbols_count + ghost_wire.from_index;
                    real_wire.to_index = base_symbols_count + ghost_wire.to_index;

                    real_wire.from_port = ghost_wire.from_port;
                    real_wire.to_port = ghost_wire.to_port;
                    real_wire.creation_mode = ghost_wire.creation_mode;
                    real_wire.selected = false;

                    vec_wires_model.push(real_wire);
                }
            }
        }

        ui.set_ghost_symbols(slint::ModelRc::new(slint::VecModel::default()));
        ui.set_ghost_connections(slint::ModelRc::new(slint::VecModel::default()));
    });
    let history_stack = Rc::new(RefCell::new(Vec::<CanvasStateSnapshot>::new()));
    let redo_stack = Rc::new(RefCell::new(Vec::<CanvasStateSnapshot>::new())); // Your initialization

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

            r_stack.borrow_mut().clear(); // <--- Uses the cloned pointer safely!
        })
    };

    let tool_selection = current_tool.clone();
    ui.on_select_library_symbol(move |name| {
        *tool_selection.borrow_mut() = name.to_string();
    });

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
            is_selected: false,
        });
    });

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

                    println!(
                        "Line drawn from Node {}/{} to Node {}/{}",
                        final_from_index, from_port_shared, final_to_index, to_port_shared
                    );
                }

                wire_source.set(-1);
                *src_port.borrow_mut() = String::new();
            }
        }
    });

    ui.on_select_connection({
        let connections = connections.clone();
        move |index| {
            if index < 0 {
                println!("Connection unselected or cleared.");
                return;
            }
            for i in 0..connections.row_count() {
                if let Some(mut conn) = connections.row_data(i) {
                    conn.selected = i == index as usize;
                    connections.set_row_data(i, conn);
                }
            }
            println!("Selected connection {}", index);
        }
    });

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
                    label: if item.label.is_empty() {
                        item.name.to_string()
                    } else {
                        item.label.to_string()
                    },
                    node_type: item.node_type.to_string(),
                    x_pos: item.x,
                    y_pos: item.y,
                    value: item.value.to_string(),
                    bg_color: if item.bg_color.is_empty() {
                        "#0d6efd".to_string()
                    } else {
                        item.bg_color.to_string()
                    },
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
                    to_port: conn.to_port.to_string(),
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
                        let message = slint::SharedString::from(format!(
                            "Flow configuration successfully saved to:\n{}",
                            filename
                        ));
                        ui_active.invoke_trigger_alert(message);
                    }
                }
                Err(e) => {
                    if let Some(ui_active) = ui_save_weak.upgrade() {
                        let message =
                            slint::SharedString::from(format!("Failed to create file:\n{:?}", e));
                        ui_active.invoke_trigger_alert(message);
                    }
                }
            }
        }
    });

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
                    let msg = slint::SharedString::from(format!(
                        "Failed to parse layout configuration file:\n{:?}",
                        e
                    ));
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
                is_selected: false,
            });
        }

        for wire in decoded_flow.wires {
            let from_id_str = wire.from.replace("node_", "");
            let to_id_str = wire.to.replace("node_", "");

            if let (Ok(from_val), Ok(to_val)) =
                (from_id_str.parse::<i32>(), to_id_str.parse::<i32>())
            {
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
            let message = slint::SharedString::from(format!(
                "Flow composition parsed successfully!\nLoaded File: {}",
                filename
            ));
            ui_active.invoke_trigger_alert(message);
        }
    });

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

    let clear_model = symbols_model.clone();
    let clear_connections = connections.clone();
    let save_h = save_history.clone();
    ui.on_clear_canvas(move || {
        save_h(); // Capture history framework stack frame state before total wipeouts
        clear_model.set_vec(vec![]);
        clear_connections.set_vec(vec![]);
    });

    let undo_symbols = symbols_model.clone();
    let undo_connections = connections.clone();
    let undo_stack = history_stack.clone();
    let r_stack = redo_stack.clone(); // Kept variable name clean

    ui.on_undo_action(move || {
        let mut u_stack = undo_stack.borrow_mut();
        let mut redo_stk = r_stack.borrow_mut();

        if let Some(previous_state) = u_stack.pop() {
            let current_state = CanvasStateSnapshot {
                symbols: undo_symbols.iter().collect(),
                connections: undo_connections.iter().collect(),
            };

            redo_stk.push(current_state);

            undo_symbols.set_vec(previous_state.symbols);
            undo_connections.set_vec(previous_state.connections);

            println!(
                "Undo executed. Undo stack: {}, Redo stack: {}",
                u_stack.len(),
                redo_stk.len()
            );
        } else {
            println!("No historical snapshot items left to restore.");
        }
    });

    let redo_symbols = symbols_model.clone();
    let redo_connections = connections.clone();
    let undo_stack = history_stack.clone();
    let r_stack = redo_stack.clone();

    ui.on_redo_action(move || {
        let mut u_stack = undo_stack.borrow_mut();
        let mut redo_stk = r_stack.borrow_mut();

        if let Some(next_state) = redo_stk.pop() {
            let current_state = CanvasStateSnapshot {
                symbols: redo_symbols.iter().collect(),
                connections: redo_connections.iter().collect(),
            };

            u_stack.push(current_state);

            redo_symbols.set_vec(next_state.symbols);
            redo_connections.set_vec(next_state.connections);

            println!(
                "Redo executed. Undo stack: {}, Redo stack: {}",
                u_stack.len(),
                redo_stk.len()
            );
        } else {
            println!("No actions left to redo!");
        }
    });

    ui.run()
}
