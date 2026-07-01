use slint::{ComponentHandle, Model, ModelRc, ToSharedString, VecModel, SharedString};
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
mod graph_engine;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
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
    ip_ports: i32,
    op_ports: i32
   
}

#[derive(Clone, Debug)]
struct WireClipboardData {
    from_index: i32,
    to_index: i32,
    creation_mode: i32,
    from_port: String,
    to_port: String,
    from_port_offset_x: f32,
    from_port_offset_y: f32,
    to_port_offset_x: f32,
    to_port_offset_y: f32,
}

#[derive(Clone, Debug)]
struct SubgraphClipboard {
    nodes: Vec<(usize, SymbolClipboardData)>,
    wires: Vec<WireClipboardData>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct JsonNode {
    pub id: i32,
    pub label: String,
    #[serde(rename = "node_type")]
    pub node_type: String,
    pub x_pos: f32,
    pub y_pos: f32,
    pub value: String,
    pub bg_color: String,
    pub creation_mode: i32,
    pub ip_ports: i32,
    pub op_ports: i32,
    pub ip_names: Vec<String>,
    pub ip_values: Vec<String>,
    pub op_names: Vec<String>,
    pub op_values: Vec<String>,
    pub ip_types: Vec<String>,
    pub op_types: Vec<String>,
    
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct JsonWire {
    pub from: String,
    pub to: String,
    pub layout_mode: i32,
    pub from_port: String,
    pub to_port: String,
    pub from_port_offset_x: f32,
    pub from_port_offset_y: f32,
    pub to_port_offset_x: f32,
    pub to_port_offset_y: f32,
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(main))]
pub fn main() -> Result<(), slint::PlatformError> {
    #[cfg(target_arch = "wasm32")]
    {
        // Uncomment this! It will print the ACTUAL Rust panic message
        // to your browser console instead of just saying "unreachable"
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    }
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
fn model_to_vec<T: Clone + 'static>(model: &slint::ModelRc<T>) -> Vec<T> {
    let mut vec = Vec::new();
    for i in 0..model.row_count() {
        if let Some(item) = model.row_data(i) {
            vec.push(item);
        }
    }
    vec
}


ui.on_add_ip_port({
    let ui_handle = ui.as_weak();
    move || {
        if let Some(ui) = ui_handle.upgrade() {
            
            let mut names = model_to_vec(&ui.get_ip_names());
            names.push("".into());
            ui.set_ip_names(std::rc::Rc::new(slint::VecModel::from(names)).into());
            
            let mut values = model_to_vec(&ui.get_ip_values());
            values.push("".into());
            ui.set_ip_values(std::rc::Rc::new(slint::VecModel::from(values)).into());

            let mut types = model_to_vec(&ui.get_ip_types());
            types.push("Number".into()); // Default type
            ui.set_ip_types(std::rc::Rc::new(slint::VecModel::from(types)).into());
        }
    }
});

ui.on_add_op_port({
    let ui_handle = ui.as_weak();
    move || {
        if let Some(ui) = ui_handle.upgrade() {
            // Convert using the helper
            let mut names = model_to_vec(&ui.get_op_names());
            names.push("".into());
            ui.set_op_names(std::rc::Rc::new(slint::VecModel::from(names)).into());
            
            let mut values = model_to_vec(&ui.get_op_values());
            values.push("".into());
            ui.set_op_values(std::rc::Rc::new(slint::VecModel::from(values)).into());

            let mut types = model_to_vec(&ui.get_op_types());
            types.push("Number".into()); // Default type
            ui.set_op_types(std::rc::Rc::new(slint::VecModel::from(types)).into());
        }
    }
}); 
ui.on_request_edit_node({
    let symbols = symbols_model.clone();
    let ui_handle = ui.as_weak();
    
    move |index| {
        if let Some(ui) = ui_handle.upgrade() {
            if let Some(node) = symbols.row_data(index as usize) {
                  let valid_ip_names: Vec<String> = node.ip_names.iter()
                    .map(|s| s.to_string())
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                
                let valid_op_names: Vec<String> = node.op_names.iter()
                    .map(|s| s.to_string())
                    .filter(|s| !s.trim().is_empty())
                    .collect();

              
                let ip_port_count = valid_ip_names.len() as i32;
                let op_port_count = valid_op_names.len() as i32;

               
                ui.set_ip_port_count(ip_port_count);
                ui.set_op_port_count(op_port_count);
                ui.set_ip_names(node.ip_names.clone());
                ui.set_ip_values(node.ip_values.clone());
                ui.set_ip_types(node.ip_types.clone());
                ui.set_op_names(node.op_names.clone());
                ui.set_op_values(node.op_values.clone());
                ui.set_op_types(node.op_types.clone());
                
                ui.set_edit_label(node.label.clone());
                ui.set_editing_index(index as i32);
                ui.set_show_node_modal(true);
            }
        }
    }
});
let symbols_model_for_closure = symbols_model.clone();
let connections_model = connections.clone();

ui.on_deploy_flow_clicked(move || {
       let connections_vec: Vec<Connection> = (0..connections_model.row_count())
        .filter_map(|i| connections_model.row_data(i))
        .collect();
       graph_engine::execute_deploy_flow(symbols_model_for_closure.clone(), connections_vec);

     println!("Deployment flow executed successfully.");
});
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
                        ip_ports: symbol.ip_ports,
                        op_ports: symbol.op_ports,

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
                        from_port_offset_x: wire.from_port_offset_x,
                        from_port_offset_y: wire.from_port_offset_y,
                        to_port_offset_x: wire.to_port_offset_x,
                        to_port_offset_y: wire.to_port_offset_y,
                    });
                }
            }
        }

        let total_nodes = copied_nodes.len();

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
                        ip_ports: symbol.ip_ports,
                        op_ports: symbol.op_ports,

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
                            from_port_offset_x: wire.from_port_offset_x,
                            from_port_offset_y: wire.from_port_offset_y,
                            to_port_offset_x: wire.to_port_offset_x,
                            to_port_offset_y: wire.to_port_offset_y,
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
    let symbols_drop_ref = symbols_model.clone();
    let wires_drop_ref = connections.clone();
   
    ui.on_paste_selected_subgraph(move |drop_x, drop_y| {
        if let Some(ref clipboard) = *clipboard_paste.borrow() {
            if clipboard.nodes.is_empty() {
                return;
            }

            if let Some(ui_active) = ui_weak_paste.upgrade() {
                let base_symbols_count = symbols_drop_ref.row_count() as i32;

                
                let vec_nodes_model = match symbols_drop_ref
                    .as_any()
                    .downcast_ref::<slint::VecModel<SymbolEntry>>()
                {
                    Some(m) => m,
                    None => return,
                };
                let vec_wires_model = match wires_drop_ref
                    .as_any()
                    .downcast_ref::<slint::VecModel<Connection>>()
                {
                    Some(m) => m,
                    None => return,
                };

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
                    let mut real_node = SymbolEntry::default();
                   
                    let unique_index = base_symbols_count + (local_idx as i32);
                    real_node.name = format!("node_{}", unique_index).to_shared_string();
                    real_node.label = node.label.to_shared_string();
                    real_node.node_type = node.node_type.to_shared_string();
                    real_node.bg_color = node.bg_color.to_shared_string();
                    real_node.value = node.value.to_shared_string();
                    real_node.creation_mode = node.creation_mode;
                    real_node.is_selected = false;
                    real_node.ip_ports = node.ip_ports;
                    real_node.op_ports = node.op_ports;
                    real_node.ip_names = slint::ModelRc::default();
                    real_node.op_names = slint::ModelRc::default();
                    real_node.ip_values = slint::ModelRc::default();
                    real_node.op_values = slint::ModelRc::default();                   
                    real_node.x = drop_x + (node.x - min_x);
                    real_node.y = drop_y + (node.y - min_y);

                    vec_nodes_model.push(real_node);
                }

                
                for wire in &clipboard.wires {
                    if let (Some(local_from), Some(local_to)) = (
                        local_index_map.get(&wire.from_index),
                        local_index_map.get(&wire.to_index),
                    ) {
                        let mut real_wire = Connection::default();
                        real_wire.from_index = base_symbols_count + *local_from;
                        real_wire.to_index = base_symbols_count + *local_to;
                        real_wire.from_port = wire.from_port.to_shared_string();
                        real_wire.to_port = wire.to_port.to_shared_string();
                        real_wire.creation_mode = wire.creation_mode;
                        real_wire.selected = false;
                        real_wire.from_port_offset_x = wire.from_port_offset_x;
                        real_wire.from_port_offset_y = wire.from_port_offset_y;
                        real_wire.to_port_offset_x = wire.to_port_offset_x;
                        real_wire.to_port_offset_y = wire.to_port_offset_y;
                        vec_wires_model.push(real_wire);
                    }
                }

                
                ui_active.window().request_redraw();
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

        let ghost_symbols = ui.get_ghost_symbols();
        let ghost_connections = ui.get_ghost_connections();

        if ghost_symbols.row_count() == 0 {
            return;
        }

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

                    real_node.name = format!("node_{}", i).to_shared_string();
                    real_node.label = ghost.label;
                    real_node.value = ghost.value;
                    real_node.node_type = ghost.node_type;
                    real_node.bg_color = ghost.bg_color;
                    real_node.x = drop_x + ghost.x;
                    real_node.y = drop_y + ghost.y;
                    real_node.is_selected = false; // Prevent selection loops on canvas refresh
                    real_node.creation_mode = ghost.creation_mode;
                    real_node.ip_names = slint::ModelRc::default();
                    real_node.ip_values= slint::ModelRc::default();
                    real_node.op_names= slint::ModelRc::default();
                    real_node.op_values= slint::ModelRc::default();
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

              ui.set_is_pasting_mode(false);
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
            r_stack.borrow_mut().clear(); 
        })
    };

    let tool_selection = current_tool.clone();
    ui.on_select_library_symbol(move |name| {
        *tool_selection.borrow_mut() = name.to_string();
    });

    let place_model = symbols_model.clone();
    let save_h = save_history.clone();

    ui.on_place_symbol(
        move |name, node_type, bg_color, x, y, mode, ip_ports, op_ports| {
            save_h(); // Log history block state before structural push
            let new_id = place_model.row_count() as i32;
            place_model.push(SymbolEntry {
                id: new_id,
                name: name.clone(),
                label: name.clone(),
                node_type,
                bg_color,
                x,
                y,
                value: "".into(),
                creation_mode: mode,
                is_selected: false,
                ip_ports,
                op_ports,
                ip_names: slint::ModelRc::default(),
                ip_values: slint::ModelRc::default(),
                op_names: slint::ModelRc::default(),
                op_values: slint::ModelRc::default(),
                ip_types: slint::ModelRc::default(),
                op_types: slint::ModelRc::default(),
            });
        },
    );

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
let active_source_port = Rc::new(std::cell::RefCell::new(String::new()));
let active_source_is_input = Rc::new(Cell::new(false));

ui.on_handle_port_click({
    let conn_model = connections.clone();
    let wire_source = wire_source.clone();
    let src_port = active_source_port.clone();
    let src_is_input = active_source_is_input.clone(); 
    let save_h = save_history.clone();
    let model = symbols_model.clone();
    move |node_index, is_input, mode, port_name| {
        let port_str = port_name.to_string();
        let source = wire_source.get();

        if source == -1 {
                
            wire_source.set(node_index);
            *src_port.borrow_mut() = port_str;
            src_is_input.set(is_input); 
            println!("Drag STARTED from Node {}, Port {} (Is Input: {})", node_index, port_name, is_input);
        } else {
         
            let first_is_input = src_is_input.get();
            
        
            if source != node_index && first_is_input != is_input {
                let first_port = src_port.borrow().clone();
                let second_port = port_str.clone();

             
               let (final_from_index, final_to_index, final_from_port, final_to_port) = if first_is_input {                    
                    (node_index, source, second_port.clone(), first_port.clone())
                } else {                   
                    (source, node_index, first_port.clone(), second_port.clone())
                };

                let from_node = model.row_data(final_from_index as usize).unwrap();
                let to_node = model.row_data(final_to_index as usize).unwrap();

                let from_ip_count = from_node.ip_ports as usize;
                let to_ip_count = to_node.ip_ports as usize;
                let (from_off_x, from_off_y) =
                calculate_port_offsets(&final_from_port, from_ip_count);

                let (to_off_x, to_off_y) =
                calculate_port_offsets(&final_to_port, to_ip_count);
                let from_port_shared = slint::SharedString::from(final_from_port);
                let to_port_shared = slint::SharedString::from(final_to_port);
                save_h(); 
                println!(
                    "Line drawn from Node {}/{} to Node {}/{}",
                    final_from_index, from_port_shared, final_to_index, to_port_shared
                );
                conn_model.push(Connection {
                    from_index: final_from_index,
                    to_index: final_to_index,
                    selected: false,
                    creation_mode: mode,
                    from_port: from_port_shared,
                    to_port: to_port_shared,
                    from_port_offset_x: from_off_x,
                    from_port_offset_y: from_off_y,
                    to_port_offset_x: to_off_x,
                    to_port_offset_y: to_off_y,
                });
            } else if first_is_input == is_input && source != node_index {
                println!("Connection rejected: Cannot connect two identical port types (IP->IP or OP->OP).");
            }                   
                   
            wire_source.set(-1);
            *src_port.borrow_mut() = String::new();
            src_is_input.set(false);
        }
    }
});

fn calculate_port_offsets(port_name: &str, ip_ports_total: usize) -> (f32, f32) {
    let parts: Vec<&str> = port_name.split('_').collect();
    let side = parts[0];
    let port_index: usize = parts[1].parse().unwrap_or(0);
    
    let header_height = 40.0;
    let row_height = 35.0;

    match side {
        "left" => (0.0, header_height + (port_index as f32 * row_height) + (row_height / 2.0)),
        "right" => (140.0, header_height + (ip_ports_total as f32 * row_height) + (port_index as f32 * row_height) + (row_height / 2.0)),
        _ => (0.0, 0.0)
    }
}
ui.on_select_connection({
        let connections = connections.clone();
        move |index| {
            if index < 0 {
                for i in 0..connections.row_count() {
                    if let Some(mut conn) = connections.row_data(i) {
                        conn.selected = false;
                        connections.set_row_data(i, conn);
                    }
                }
                println!("All connections deselected.");
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
    let ui_handle = ui.as_weak();
    let conn_model = connections.clone();

    move |index, _ip_count, _op_count, label, ip_names, ip_values, op_names, op_values, ip_types, op_types| {
        if let Some(ui) = ui_handle.upgrade() {

         

 for i in 0..ip_types.row_count() {
            let ty = ip_types.row_data(i).unwrap_or_default();
            let value = ip_values.row_data(i).unwrap_or_default();

            if ty.as_str() == "Number" && value.parse::<f64>().is_err() {
             
                ui.invoke_trigger_alert(
                    format!("Invalid input value at port {}", i).into()
                );
                return false;
            }
        }

      
        for i in 0..op_types.row_count() {
            let ty = op_types.row_data(i).unwrap_or_default();
            let value = op_values.row_data(i).unwrap_or_default();

            if ty.as_str() == "Number" && value.parse::<f64>().is_err() {
                ui.invoke_trigger_alert(
                   
                    format!("Invalid output value at port {}", i).into()
                );
                return false;
            }
        }

   
            
            if let Some(mut node) = model.row_data(index as usize) {
                save_h();

                let ip_n: Vec<slint::SharedString> = (0..ip_names.row_count())
                    .map(|i| ip_names.row_data(i).unwrap_or_default())
                    .filter(|s| !s.trim().is_empty())
                    .collect();

                let op_n: Vec<slint::SharedString> = (0..op_names.row_count())
                    .map(|i| op_names.row_data(i).unwrap_or_default())
                    .filter(|s| !s.trim().is_empty())
                    .collect();

                node.label = label;
                node.ip_ports = ip_n.len() as i32;
                node.op_ports = op_n.len() as i32;

                node.ip_names = std::rc::Rc::new(slint::VecModel::from(ip_n)).into();
                node.op_names = std::rc::Rc::new(slint::VecModel::from(op_n)).into();

                node.ip_values = ip_values;
                node.op_values = op_values;
                node.ip_types = ip_types;
                node.op_types = op_types;
    for i in 0..conn_model.row_count() {
    let mut conn = conn_model.row_data(i).unwrap();
    
    if conn.from_index == index {
        let (x, y) = calculate_port_offsets(&conn.from_port, node.ip_ports as usize);
        conn.from_port_offset_y = y;
        conn_model.set_row_data(i, conn);
    }
}
                model.set_row_data(index as usize, node);
            
                ui.set_editing_index(-1);
                return  true;
             
             
            }
        }
        return  false;
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
         
                json_nodes.push(JsonNode {
                    id: item.id,
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
                    ip_ports: item.ip_ports,
                    op_ports: item.op_ports,
                    
                    ip_names: item.ip_names
    .iter()
    .map(|s| s.to_string()) 
    .collect(),
    ip_values: item.ip_values
    .iter()
    .map(|s| s.to_string()) 
    .collect(),
    op_names: item.op_names
    .iter()
    .map(|s| s.to_string()) 
    .collect(),
    op_values:item.op_values
    .iter()
    .map(|s| s.to_string()) 
    .collect(),
    ip_types:item.ip_types
    .iter()
    .map(|s| s.to_string()) 
    .collect(),
     op_types:item.op_types
    .iter()
    .map(|s| s.to_string()) 
    .collect(),
                });
            }
        }

        let mut json_wires = Vec::new();
        for i in 0..export_connections_active.row_count() {
            if let Some(conn) = export_connections_active.row_data(i) {
                json_wires.push(JsonWire {
                    from: format!("node_{}", conn.from_index ),
                    to: format!("node_{}", conn.to_index ),
                    layout_mode: conn.creation_mode,
                    from_port: conn.from_port.to_string(),
                    to_port: conn.to_port.to_string(),
                    from_port_offset_x: conn.from_port_offset_x,
                    from_port_offset_y: conn.from_port_offset_y,
                    to_port_offset_x: conn.to_port_offset_x,
                    to_port_offset_y: conn.to_port_offset_y,
                    
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
            // --- WASM (Web Browser) Target ---
            #[cfg(target_arch = "wasm32")]
            {
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        if let Ok(Some(element)) = document
                            .create_element("a")
                            .map(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok())
                        {
                            // 1. Create a JS array containing our JSON string
                            let parts =
                                js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(&json_string));

                            // 2. Specify the MIME type as application/json
                            let mut options = web_sys::BlobPropertyBag::new();
                            options.type_("application/json");

                            // 3. Create a Blob and generate a temporary download URL
                            if let Ok(blob) =
                                web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)
                            {
                                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                                    let _ = element.set_attribute("href", &url);
                                    let _ = element.set_attribute("download", &filename);

                                    // 4. Trigger the download dialogue
                                    element.click();

                                    // 5. Clean up the URL resource from browser memory
                                    let _ = web_sys::Url::revoke_object_url(&url);

                                    if let Some(ui_active) = ui_save_weak.upgrade() {
                                        let message = slint::SharedString::from(format!(
                                            "Flow configuration prepared for download:\n{}",
                                            filename
                                        ));
                                        ui_active.invoke_trigger_alert(message);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
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
                            let message = slint::SharedString::from(format!(
                                "Failed to create file:\n{:?}",
                                e
                            ));
                            ui_active.invoke_trigger_alert(message);
                        }
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
        let import_symbols = import_symbols.clone();
        let import_connections = import_connections.clone();
        let ui_load_weak = ui_load_weak.clone();
        let save_h = save_h.clone();

        let fut = async move {
            let file_picker = rfd::AsyncFileDialog::new()
                .add_filter("JSON Flow Profiles", &["json"])
                .set_title("Select Flow Configuration Layout")
                .pick_file()
                .await; //  This is now legal because it's inside the `async move` block!

            let file_handle = match file_picker {
                Some(handle) => handle,
                None => return,
            };

            let file_bytes = file_handle.read().await;
            let file_data = match String::from_utf8(file_bytes) {
                Ok(s) => s,
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

            save_h();
            let mut fresh_symbols = Vec::new();
            let mut fresh_connections = Vec::new();

            for node in decoded_flow.nodes {
                fresh_symbols.push(SymbolEntry {
                    id: node.id,
                    name: node.label.clone().into(),
                    label: node.label.into(),
                    node_type: node.node_type.into(),
                    x: node.x_pos,
                    y: node.y_pos,
                    value: node.value.into(),
                    bg_color: node.bg_color.into(),
                    creation_mode: node.creation_mode,
                    is_selected: false,
                    ip_ports: node.ip_ports,
                    op_ports: node.op_ports,

                    ip_names: ModelRc::new(VecModel::from(node.ip_names.into_iter().map(SharedString::from).collect::<Vec<SharedString>>(),)),
                    ip_values: ModelRc::new(VecModel::from(node.ip_values.into_iter().map(SharedString::from).collect::<Vec<SharedString>>(),)),
                    op_names: ModelRc::new(VecModel::from(node.op_names.into_iter().map(SharedString::from).collect::<Vec<SharedString>>(),)),
                    op_values: ModelRc::new(VecModel::from(node.op_values.into_iter().map(SharedString::from).collect::<Vec<SharedString>>(),)),
                    ip_types: ModelRc::new(VecModel::from(node.ip_types.into_iter().map(SharedString::from).collect::<Vec<SharedString>>(),)),
                    op_types: ModelRc::new(VecModel::from(node.op_types.into_iter().map(SharedString::from).collect::<Vec<SharedString>>(),)),
                
                });
            }

            for wire in decoded_flow.wires {
                let from_id_str = wire.from.replace("node_", "");
                let to_id_str = wire.to.replace("node_", "");

                if let (Ok(from_val), Ok(to_val)) =
                    (from_id_str.parse::<i32>(), to_id_str.parse::<i32>())
                {
                    fresh_connections.push(Connection {
                        from_index: from_val,
                        to_index: to_val,
                        selected: false,
                        creation_mode: wire.layout_mode,
                        from_port: slint::SharedString::from(&wire.from_port),
                        to_port: slint::SharedString::from(&wire.to_port),
                        from_port_offset_x: wire.from_port_offset_x,
                        from_port_offset_y: wire.from_port_offset_y,
                        to_port_offset_x: wire.to_port_offset_x,
                        to_port_offset_y: wire.to_port_offset_y                        
                    });
                }
            }

            import_symbols.set_vec(fresh_symbols);
            import_connections.set_vec(fresh_connections);

            if let Some(ui_active) = ui_load_weak.upgrade() {
                let filename = file_handle.file_name();
                let message = slint::SharedString::from(format!(
                    "Flow composition parsed successfully!\nLoaded File: {}",
                    filename
                ));
                ui_active.invoke_trigger_alert(message);
            }
        }; 
        
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(fut); // Runs asynchronously on the browser event thread
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            slint::spawn_local(fut).unwrap();
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
    let delete_model = symbols_model.clone();
    let ui_weak = ui.as_weak();
    let save_h = save_history.clone();

    ui.on_delete_selected_symbols({
        let connections = connections.clone();
        let delete_model = delete_model.clone();
        let save_h = save_h.clone();

        move |indexes_model| {
            if let Some(ui) = ui_weak.upgrade() {
                // 1. Cleanly parse indices, ignoring any -1 wrapping errors
                let mut selected_indices: Vec<i32> =
                    indexes_model.iter().filter(|&idx| idx >= 0).collect();

                if selected_indices.is_empty() {
                    return;
                }

                save_h(); 
              
                for i in (0..connections.row_count()).rev() {
                    if let Some(mut conn) = connections.row_data(i) {
                          let from_deleted = selected_indices.contains(&conn.from_index);
                        let to_deleted = selected_indices.contains(&conn.to_index);

                        if from_deleted || to_deleted {
                            connections.remove(i);
                            println!("Removed broken connection at index {}", i);
                        } else {
                            let mut changed = false;

                            // Instead of -= 1,
                            let shift_from = selected_indices
                                .iter()
                                .filter(|&&idx| conn.from_index > idx)
                                .count() as i32;
                            if shift_from > 0 {
                                conn.from_index -= shift_from;
                                changed = true;
                            }

                            let shift_to = selected_indices
                                .iter()
                                .filter(|&&idx| conn.to_index > idx)
                                .count() as i32;
                            if shift_to > 0 {
                                conn.to_index -= shift_to;
                                changed = true;
                            }

                            if changed {
                                connections.set_row_data(i, conn);
                            }
                        }
                    }
                }

                              
                selected_indices.sort_by(|a, b| b.cmp(a));

              
                for &index in &selected_indices {
                    let index_usize = index as usize;
                    if index_usize < delete_model.row_count() {
                        delete_model.remove(index_usize);
                        println!("Deleted symbol at index {}", index_usize);
                    }
                }

            
                let empty_model = std::rc::Rc::new(slint::VecModel::default()).into();
                ui.set_selected_indexes(empty_model);

                let msg = format!("Deleted {} nodes.", selected_indices.len());
                ui.invoke_trigger_alert(msg.into());
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
