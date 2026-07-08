use crate::{AppWindow, Connection, MQTT_TX, MqttCommand, SymbolEntry, UI_UPDATE_TX};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use slint::Weak;
use slint::{Model, ModelRc, SharedString, VecModel};

pub fn start(ui: Weak<AppWindow>) {
    let (ui_tx, mut ui_rx) = tokio::sync::mpsc::channel::<(String, String)>(32);

    let (mqtt_tx, mut mqtt_rx) = tokio::sync::mpsc::channel::<MqttCommand>(32);

    MQTT_TX.set(mqtt_tx).unwrap();
    UI_UPDATE_TX.set(ui_tx).unwrap();

    tokio::spawn(async move {
        let mqttoptions = MqttOptions::new("rust-ui-client", "localhost", 1883);
        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

        loop {
            tokio::select! {
                            Some(cmd) = mqtt_rx.recv() => {
                        match cmd {
                        MqttCommand::Publish { topic, payload } => {
                        let result = client
                            .publish(topic, QoS::AtMostOnce, false, payload)
                            .await;

                        match result {
                            Ok(_) => {
                                println!("DEBUG: Message successfully pushed to network socket.");
                            }
                            Err(e) => {
                                eprintln!("DEBUG: Failed to push message to network: {:?}", e);
                            }
                        }
                    }

                    MqttCommand::Subscribe { topic } => {
                        let result = client
                            .subscribe(topic.clone(), QoS::AtMostOnce)
                            .await;

                        match result {
                            Ok(_) => {
                                println!("DEBUG: Subscribed to {}", topic);
                            }
                            Err(e) => {
                                eprintln!("DEBUG: Failed to subscribe to {}: {:?}", topic, e);
                            }
                        }
                    }
                }
            }

                    notification = eventloop.poll() => {
                                match notification {
                                    Ok(Event::Incoming(Packet::Publish(p))) => {
                                        let topic = p.topic.to_string();
                                        let payload = String::from_utf8_lossy(&p.payload).to_string();

                                        println!("DEBUG: Received message from MQTT broker on {}: {}", topic, payload);

                                        let ui_tx = UI_UPDATE_TX
                                            .get()
                                            .expect("UI_UPDATE_TX not initialized");

                                        let _ = ui_tx.send((topic, payload)).await;
                                    }

                                    Ok(_) => {

                                    }

                                    Err(e) => {
                                        eprintln!("DEBUG: MQTT Eventloop Error: {:?}", e);

                                        tokio::time::sleep(
                                            tokio::time::Duration::from_secs(1)
                                        )
                                        .await;
                                    }
                                }
                            }
                }
        }
    });
    tokio::spawn(async move {
        while let Some((topic, payload)) = ui_rx.recv().await {
            let weak_ui = ui.clone();
            let topic = topic.clone();
            let payload = payload.clone();

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak_ui.upgrade() {
                    let symbols_model = ui.get_all_symbols();
                    for i in 0..symbols_model.row_count() {
                        if let Some(mut node) = symbols_model.row_data(i) {
                            if node.node_type == "mqtt_in" && node.mqtt_topic == topic {
                                println!("MQTT Received -> Topic: {}, Payload: {}", topic, payload);

                                if let Some(vec_model) = node
                                    .op_values
                                    .as_any()
                                    .downcast_ref::<VecModel<SharedString>>()
                                {
                                    vec_model.set_row_data(0, payload.clone().into());
                                    symbols_model.set_row_data(i, node);
                                }
                            }
                        }
                    }

                    let mut nodes: Vec<SymbolEntry> = (0..symbols_model.row_count())
                        .filter_map(|i| symbols_model.row_data(i))
                        .collect();

                    let connections_model = ui.get_connections();

                    let connections_vec: Vec<Connection> = (0..connections_model.row_count())
                        .filter_map(|i| connections_model.row_data(i))
                        .collect();

                    for wire in connections_vec {
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
                                println!(
                                    "Propagating '{}' -> {}:{}",
                                    val, nodes[dst_i].label, dst_port
                                );

                                let mut ip_vals: Vec<SharedString> = (0..nodes[dst_i]
                                    .ip_values
                                    .row_count())
                                    .map(|i| nodes[dst_i].ip_values.row_data(i).unwrap_or_default())
                                    .collect();

                                if dst_port < ip_vals.len() {
                                    ip_vals[dst_port] = val;

                                    nodes[dst_i].ip_values = ModelRc::new(VecModel::from(ip_vals));

                                    // IMPORTANT: Update the UI model
                                    symbols_model.set_row_data(dst_i, nodes[dst_i].clone());
                                }
                            }
                        }
                    }
                }
            })
            .unwrap();
        }
    });
}
