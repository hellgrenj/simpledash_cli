mod cli;
mod client;
mod models;

use std::net::TcpStream;

use cli::clear_screen;
use cli_table::{format::Justify, Cell, Style, Table};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Select};
use models::{ClusterInfo, Payload};
use tungstenite::{stream::MaybeTlsStream, WebSocket};

fn main() {
    ctrlc::set_handler(move || {
        cli::clean_exit();
    })
    .expect("Error setting Ctrl-C handler");
    clear_screen();
    println!("{}", "..::simpledash CLI::..\n".magenta().on_black().bold());
    let settings = cli::parse_args();
    let mut socket = client::connect_to_host(&settings.host).expect("Error connecting to host");
    let cluster_info =
        client::get_cluster_info(&settings.host).expect("Failed to fetch Simpledash Context");

    let peek_payload = visualize_cluster_status(&mut socket, &settings.host);
    let ns = select_namespace(&cluster_info);
    visualize_payload(peek_payload, &ns, &cluster_info);

    loop {
        let payload_option = match receive_payload(&mut socket, &settings.host) {
            Ok(payload) => payload,
            Err(e) => {
                eprintln!("Error receiving payload: {:?}", e);
                continue;
            }
        };

        let payload = match payload_option {
            Some(payload) => payload,
            None => continue, // no payload on ping (tungstenite replies with pong automatically)
        };

        visualize_payload(payload, &ns, &cluster_info);
    }
}

fn visualize_cluster_status(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    host: &String,
) -> Payload {
    let (payload, status_table) = get_cluster_status(socket, host);
    println!("{}", status_table);
    payload
}

fn get_cluster_status(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    host: &String,
) -> (Payload, String) {
    loop {
        let payload_option = match receive_payload(socket, host) {
            Ok(payload) => payload,
            Err(e) => {
                eprintln!("Error receiving payload: {:?}", e);
                continue;
            }
        };
        let payload = match payload_option {
            Some(payload) => payload,
            None => continue, // no payload on ping (tungstenite replies with pong automatically)
        };

        let mut not_running_pods = Vec::new();
        for (_, value) in payload.nodes.iter() {
            for pod in value.iter() {
                if pod.status != "Running" && pod.status != "Succeeded" && pod.status != "Completed"
                {
                    not_running_pods.push(pod);
                }
            }
        }
        let mut rows = vec![vec![
            "cluster".magenta().bold().cell().bold(true),
            "#unhealthy pods".magenta().bold().cell().bold(true),
            "...in namespaces".magenta().bold().cell().bold(true),
            "overall status".magenta().bold().cell().bold(true),
        ]];
        if not_running_pods.len() > 0 {
            let failed_in_namespaces = not_running_pods
                .iter()
                .map(|pod| pod.namespace.clone())
                .collect::<std::collections::HashSet<String>>() // Collect into a HashSet to remove duplicates
                .into_iter()
                .collect::<Vec<String>>()
                .join(", ");

            rows.push(vec![
                host.blue().bold().cell().justify(Justify::Left),
                not_running_pods
                    .len()
                    .to_string()
                    .red()
                    .cell()
                    .justify(Justify::Left),
                failed_in_namespaces.cell().justify(Justify::Left),
                "BAD".bold().red().cell().justify(Justify::Left),
            ]);
        } else {
            rows.push(vec![
                host.blue().bold().cell().justify(Justify::Left),
                not_running_pods
                    .len()
                    .to_string()
                    .green()
                    .cell()
                    .justify(Justify::Left),
                "".cell().justify(Justify::Left),
                "OK".bold().green().cell().justify(Justify::Left),
            ]);
        }
        let table = rows.table().bold(true);
        
        let table_display = match table.display() {
            Ok(display) => display,
            Err(e) => {
                eprintln!("Error displaying cluster status table: {:?}", e);
                return (payload, "could not visualize cluster status".to_string());
            }
        };

        break (payload, table_display.to_string());
    }
}

fn receive_payload(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    host: &String,
) -> Result<Option<Payload>, Box<dyn std::error::Error>> {
    if !socket.can_read() {
        println!("Socket is not readable");
        println!("trying to reconnect");
        *socket = client::connect_to_host(host)?;
    }
    let read_result = socket.read()?;

    if read_result.len() > 0 {
        let payload: Payload = serde_json::from_str(&read_result.to_string())?;
        Ok(Some(payload))
    } else {
        Ok(None) // no payload on ping (tungstenite replies with pong automatically)
    }
}

fn select_namespace(cluster_info: &ClusterInfo) -> String {
    let namespaces = &cluster_info.namespaces;
    let selections = &namespaces[..];
    println!("");
    println!("select namespace:({})", selections.len());
    println!("");
    let selection_result = Select::with_theme(&ColorfulTheme::default())
        .default(0)
        .items(&selections[..])
        .interact();

    let selection = match selection_result {
        Ok(selection) => selection,
        Err(e) => {
            eprintln!("Error selecting namespace: {:?}", e);
            return "default_namespace".to_string();
        }
    };
    namespaces[selection].to_string()
}

fn visualize_payload(payload: Payload, namespace: &str, cluster_info: &ClusterInfo) {
    clear_screen();
    print_endpoints(&payload, namespace);
    print_deployments(&payload, namespace, cluster_info);
    print_pods_table(&payload, namespace, cluster_info);
}

fn print_endpoints(payload: &Payload, namespace: &str) {
    println!("{}", "Endpoints:".magenta().bold());
    println!("{}", get_endpoints_visualization(payload, namespace));
}
fn get_endpoints_visualization(payload: &Payload, namespace: &str) -> String {
    let mut result = String::new();
    if let Some(ingresses) = &payload.ingresses {
        for ingress in ingresses {
            if ingress.namespace != *namespace {
                continue;
            }
            result.push_str(&format!(
                "{}{} ({})\n",
                "https://".bold().blue(),
                ingress.endpoint.bold().blue(),
                ingress.ip
            ));
        }
    }
    result
}
fn print_deployments(payload: &Payload, namespace: &str, cluster_info: &ClusterInfo) {
    println!("{}", "Deployments:".magenta().bold());
    println!(
        "{}",
        get_deployments_visualization(payload, namespace, cluster_info)
    );
}
fn get_deployments_visualization(
    payload: &Payload,
    namespace: &str,
    cluster_info: &ClusterInfo,
) -> String {
    let mut result = String::new();
    for deployment in payload.deployments.iter() {
        if deployment.namespace != *namespace {
            continue;
        }
        if cluster_info.deployment_logs_link_enabled {
            let link_url = cluster_info
                .deployment_logs_link
                .replace("DEPLOYMENT_NAME_PLACEHOLDER", &deployment.name)
                .replace("DEPLOYMENT_NAMESPACE_PLACEHOLDER", &deployment.namespace);
            result.push_str(&format!(
                "{} ({}/{}) {}\n",
                deployment.name,
                deployment.ready_replicas,
                deployment.replicas,
                cli::make_link(link_url, "view logs".to_string())
                    .bold()
                    .blue()
            ));
        } else {
            result.push_str(&format!(
                "{} ({}/{})\n",
                deployment.name, deployment.ready_replicas, deployment.replicas
            ));
        }
    }
    result
}
fn print_pods_table(payload: &Payload, namespace: &str, cluster_info: &ClusterInfo) {
    println!("{}", "Pods:".magenta().bold());
    println!("{}", get_pods_visualization(payload, namespace));
    println!(
        "{} in {} as per {}",
        namespace.bold().magenta().on_black(),
        cluster_info.cluster_name.bold().magenta().on_black(),
        payload.timestamp.bold().yellow()
    );
}
fn get_pods_visualization(payload: &Payload, namespace: &str) -> String {
    let mut pod_rows = vec![vec![
        "node".cell().bold(true),
        "pod name".cell().bold(true),
        "status".cell().bold(true),
        "tag".cell().bold(true),
    ]];
    for (key, value) in payload.nodes.iter() {
        for pod in value.iter() {
            if pod.namespace != *namespace {
                continue;
            }

            let colored_status: ColoredString;
            if pod.status == "Running" || pod.status == "Succeeded" || pod.status == "Completed" {
                colored_status = pod.status.green();
            } else if pod.status == "Pending"
                || pod.status == "ContainerCreating"
                || pod.status == "PodInitializing"
            {
                colored_status = pod.status.yellow();
            } else {
                colored_status = pod.status.red();
            }

            let pod_image_tag = match pod.image.split(":").last() {
                Some(tag) => tag,
                None => "unknown",
            };

            pod_rows.push(vec![
                key.cell(),
                pod.name.clone().cell(),
                colored_status.cell().justify(Justify::Right),
                pod_image_tag.cell(),
            ]);
        }
    }
    let pod_table = pod_rows.table().bold(true);
    let pod_table_display = match pod_table.display() {
        Ok(display) => display,
        Err(e) => {
            eprintln!("Error displaying pod table: {:?}", e);
            return "could not visualize pods".to_string();
        }
    };
    pod_table_display.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{ClusterInfo, Deployment};

    #[test]
    fn test_get_deployments_visualization_visualizing_only_selected_namespace() {
        // Arrange
        let payload = Payload {
            deployments: vec![
                Deployment {
                    name: "deployment1".to_string(),
                    namespace: "namespace1".to_string(),
                    ready_replicas: 2,
                    replicas: 3,
                    ..Default::default()
                },
                Deployment {
                    name: "deployment2".to_string(),
                    namespace: "namespace2".to_string(),
                    ready_replicas: 1,
                    replicas: 1,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let cluster_info = ClusterInfo {
            deployment_logs_link_enabled: false,
            ..Default::default()
        };

        // Act
        let visualization = get_deployments_visualization(&payload, "namespace1", &cluster_info);

        // Assert
        // contains this
        assert!(visualization.contains("deployment1 (2/3)"));
        // and not this...
        assert!(!visualization.contains("deployment2 (1/1)"));
    }
}
