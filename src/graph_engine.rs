use crate::Connection;
use crate::SymbolEntry; // Ensure this points to your generated/defined struct
use slint::{Model, ModelRc, SharedString, VecModel};
use std::rc::Rc; // Ensure this points to your Connection struct

pub fn execute_deploy_flow(symbols_model: Rc<VecModel<SymbolEntry>>, wires: Vec<Connection>) {
    let mut nodes: Vec<SymbolEntry> = (0..symbols_model.row_count())
        .filter_map(|i| symbols_model.row_data(i))
        .collect();

    for wire in &wires {
     
        let src_id = wire.from_index;
        let dst_id = wire.to_index;

       
        let src_port_idx: usize = wire
            .from_port
            .split('_')
            .last()
            .unwrap()
            .parse()
            .unwrap_or(0);
        let dst_port_idx: usize = wire.to_port.split('_').last().unwrap().parse().unwrap_or(0);

       
        if let Some(src_node) = nodes.iter().find(|n| n.id == src_id) {
            if let Some(val) = src_node.op_values.row_data(src_port_idx) {
              
                if let Some(dst_node) = nodes.iter_mut().find(|n| n.id == dst_id) {
                    let mut ip_vals: Vec<SharedString> = (0..dst_node.ip_values.row_count())
                        .filter_map(|i| dst_node.ip_values.row_data(i))
                        .collect();

                    if dst_port_idx < ip_vals.len() {
                        ip_vals[dst_port_idx] = val;
                        dst_node.ip_values = ModelRc::new(VecModel::from(ip_vals));
                    }
                }
            }
        }
    }

    if let Some(add_node) = nodes.iter_mut().find(|n| n.id == 1) {
        let vals: Vec<i32> = (0..add_node.ip_values.row_count())
            .filter_map(|i| add_node.ip_values.row_data(i)?.parse::<i32>().ok())
            .collect();

        let mut op_results = vec![SharedString::from(""), SharedString::from("")];
        if vals.len() >= 1 {
        
            op_results[0] = vals.iter().sum::<i32>().to_string().into();
        } else {
            op_results[1] = "Error: Missing inputs".into();
        }
        add_node.op_values = ModelRc::new(VecModel::from(op_results.clone()));

        for node in nodes.iter_mut() {
            if node.node_type == "ui_output" {
                node.ip_values = ModelRc::new(VecModel::from(vec![op_results[0].clone()]));
            } else if node.node_type == "ui_error" {
                node.ip_values = ModelRc::new(VecModel::from(vec![op_results[1].clone()]));
            }
        }
    }

   
    symbols_model.set_vec(nodes);
}
