use crate::Connection;
use crate::SymbolEntry;
use slint::{Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;

pub async fn execute_deploy_flow(symbols_model: Rc<VecModel<SymbolEntry>>, wires: Vec<Connection>) {
    let mut nodes: Vec<SymbolEntry> = (0..symbols_model.row_count())
        .filter_map(|i| symbols_model.row_data(i))
        .collect();
    
    for _ in 0..nodes.len() {
        for node in nodes.iter_mut() {
            if node.node_type == "ui_function" {
                let sum: i32 = (0..node.ip_values.row_count())
                    .filter_map(|i| {
                        node.ip_values
                            .row_data(i)
                            .unwrap_or_default()
                            .parse::<i32>()
                            .ok()
                    })
                    .sum();

                let mut op_vals: Vec<SharedString> = (0..node.op_values.row_count())
                    .map(|i| node.op_values.row_data(i).unwrap_or_default())
                    .collect();

                if !op_vals.is_empty() {
                    op_vals[0] = sum.to_string().into();
                    node.op_values = ModelRc::new(VecModel::from(op_vals));
                }
            }
        }

        for wire in &wires {
            let src_idx = nodes.iter().position(|n| n.id == wire.from_index);
            let dst_idx = nodes.iter().position(|n| n.id == wire.to_index);

            if let (Some(src_i), Some(dst_i)) = (src_idx, dst_idx) {
                let src_port: usize = wire
                    .from_port
                    .split('_')
                    .last()
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);
                let dst_port: usize = wire
                    .to_port
                    .split('_')
                    .last()
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);

                if let Some(val) = nodes[src_i].op_values.row_data(src_port) {
                    let mut ip_vals: Vec<SharedString> = (0..nodes[dst_i].ip_values.row_count())
                        .map(|i| nodes[dst_i].ip_values.row_data(i).unwrap_or_default())
                        .collect();

                    if dst_port < ip_vals.len() {
                        ip_vals[dst_port] = val;
                        nodes[dst_i].ip_values = ModelRc::new(VecModel::from(ip_vals));
                    }
                }
            }
        }
    }

    for node in nodes.iter() {
        if node.node_type == "mqtt_in" {
            if let Some(tx) = crate::MQTT_TX.get() {
                let cmd = crate::MqttCommand::Subscribe {
                    topic: node.mqtt_topic.to_string(),
                };

                if let Err(e) = tx.send(cmd).await {
                    eprintln!("Failed to send Subscribe command: {:?}", e);
                }
                println!("DEBUG: Subscribe Successfully");
            } else {
                println!("DEBUG: MQTT_TX channel not initialized!");
            }
        }
    }
    for node in nodes.iter() {
        if node.node_type == "mqtt_out" {
            println!(
                "Value is {}",
                node.ip_values.row_data(0).unwrap_or_default()
            );
            if let Some(val) = node.ip_values.row_data(0) {
                let topic_string = node.mqtt_topic.to_string();

                if let Some(tx) = crate::MQTT_TX.get() {
                    let cmd = crate::MqttCommand::Publish {
                        topic: topic_string,
                        payload: val.to_string(),
                    };

                    let res = tx.send(cmd).await;
                    println!("DEBUG: Publish Successfully");
                    if let Err(e) = res {
                        println!("Failed to send MQTT command: {:?}", e);
                    }
                } else {
                    println!("DEBUG: Error - MQTT_TX channel not initialized!");
                }
            }
        }
    }

    symbols_model.set_vec(nodes);
}
