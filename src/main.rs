mod cli;
mod client;
mod models;
use cli::clear_screen;
use cli_table::{format::Justify, Cell, Style, Table};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Select};
use models::{ClusterInfo, Payload, Pods};
use std::{collections::HashMap, net::TcpStream};
use tungstenite::{stream::MaybeTlsStream, WebSocket};

fn main() {
    let settings = cli::parse_args();
    let mut socket = client::connect_to_host(&settings.host).expect("Error connecting to host");
    let cluster_info =
        client::get_cluster_info(&settings.host).expect("Failed to fetch Simpledash Context");

    clear_screen();
    let peek_payload = visualize_cluster_status(&mut socket, &settings.host);
    let ns = select_namespace(&cluster_info);
    visualize_payload(peek_payload, &ns, &cluster_info);

    loop {
        let payload = match receive_payload(&mut socket, &settings.host) {
            Ok(p) => match p {
                Some(p) => p,
                None => continue, // no payload on ping (tungstenite replies with pong automatically)
            },
            Err(e) => {
                eprintln!("Error receiving payload: {:?}", e);
                continue;
            }
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
        let payload = match receive_payload(socket, host) {
            Ok(p) => match p {
                Some(p) => p,
                None => continue, // no payload on ping (tungstenite replies with pong automatically)
            },
            Err(e) => {
                eprintln!("Error receiving payload: {:?}", e);
                continue;
            }
        };
        break check_cluster_status(host, payload);
    }
}

fn check_cluster_status(host: &String, payload: Payload) -> (Payload, String) {
    let mut pods_in_bad_state = Vec::new();
    for (_, value) in payload.nodes.iter() {
        for pod in value.iter() {
            if pod.status != "Running" && pod.status != "Succeeded" && pod.status != "Completed" {
                pods_in_bad_state.push(pod);
            }
        }
    }
    let total_pods = payload
        .nodes
        .values()
        .map(|value| value.len())
        .sum::<usize>();

    let number_of_nodes = payload.nodes.len();

    let mut rows = vec![vec![
        "cluster".magenta().bold().cell().bold(true),
        "#unhealthy pods".magenta().bold().cell().bold(true),
        "...in namespaces".magenta().bold().cell().bold(true),
        "total pods".magenta().bold().cell().bold(true),
        "#worker nodes".magenta().bold().cell().bold(true),
        "overall status".magenta().bold().cell().bold(true),
    ]];
    if !pods_in_bad_state.is_empty() {
        let failed_in_namespaces = pods_in_bad_state
            .iter()
            .map(|pod| pod.namespace.clone())
            .collect::<std::collections::HashSet<String>>() // Collect into a HashSet to remove duplicates
            .into_iter()
            .collect::<Vec<String>>()
            .join(", ");

        rows.push(vec![
            host.blue().bold().cell().justify(Justify::Left),
            pods_in_bad_state
                .len()
                .to_string()
                .red()
                .cell()
                .justify(Justify::Left),
            failed_in_namespaces.cell().justify(Justify::Left),
            total_pods.to_string().blue().cell().justify(Justify::Left),
            number_of_nodes.to_string().blue().cell().justify(Justify::Left),
            "BAD".bold().red().cell().justify(Justify::Left),
        ]);
    } else {
        rows.push(vec![
            host.blue().bold().cell().justify(Justify::Left),
            pods_in_bad_state
                .len()
                .to_string()
                .green()
                .cell()
                .justify(Justify::Left),
            "".cell().justify(Justify::Left),
            total_pods.to_string().blue().cell().justify(Justify::Left),
            number_of_nodes.to_string().blue().cell().justify(Justify::Left),
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

    (payload, table_display.to_string())
}

fn receive_payload(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    host: &str,
) -> Result<Option<Payload>, Box<dyn std::error::Error>> {
    if !socket.can_read() {
        println!("lost connection, trying to reconnect in 3 seconds...");
        std::thread::sleep(std::time::Duration::from_secs(3));
        *socket = client::connect_to_host(host)?;
    }
    let read_result = socket.read()?;

    if !read_result.is_empty() {
        let payload: Payload = serde_json::from_str(&read_result.to_string())?;
        Ok(Some(payload))
    } else {
        Ok(None) // no payload on ping (tungstenite replies with pong automatically)
    }
}

fn select_namespaces_group(cluster_info: &ClusterInfo) -> (String, Vec<String>) {
    let namespaces = &cluster_info.namespaces;
    println!("\n{} namespaces sorted by first letter", namespaces.len());
    let grouped_namespaces = group_ns_by_first_letter(namespaces.to_vec());
    let mut sorted_keys = grouped_namespaces.keys().collect::<Vec<&char>>();
    sorted_keys.sort();
    let selections = sorted_keys
        .iter()
        .map(|key| format!("{} ({})", key, grouped_namespaces.get(key).unwrap().len()))
        .collect::<Vec<String>>();
    let selection_result = Select::with_theme(&ColorfulTheme::default())
        .default(0)
        .items(&selections[..])
        .interact();

    let selection = match selection_result {
        Ok(selection) => selection,
        Err(e) => {
            eprintln!("Error selecting namespace group: {:?}", e);
            return ("_".to_string(), vec!["default_namespace".to_string()]);
        }
    };

    let key = match selections[selection].chars().next() {
        Some(key) => key,
        None => {
            eprintln!("Error selecting namespace group: {:?}", "key is None");
            return ("_".to_string(), vec!["default_namespace".to_string()]);
        }
    };
    (
        key.to_string(),
        grouped_namespaces.get(&key).unwrap().to_vec(),
    )
}

fn group_ns_by_first_letter(ns: Vec<String>) -> HashMap<char, Vec<String>> {
    let mut ns_groups: HashMap<char, Vec<String>> = HashMap::new();

    for name in ns {
        if let Some(first_letter) = name.chars().next() {
            let key = first_letter.to_ascii_uppercase();
            ns_groups.entry(key).or_default().push(name);
        }
    }
    ns_groups
}

fn select_namespace(cluster_info: &ClusterInfo) -> String {
    let (letter, namespaces) = select_namespaces_group(cluster_info);
    let selections = &namespaces[..];
    println!(
        "{} namespaces starting with '{}':",
        selections.len(),
        letter
    );
    let selection_result = Select::with_theme(&ColorfulTheme::default())
        .default(0)
        .items(selections)
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
                "https://".bold().blue(), // .. :/.. Its been our case so far that everything is TLS, simpledash server has to return the protocol in use
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
    let (pod_viz, no_pods) = get_pods_visualization(payload, namespace);
    println!("{}({})", "Pods:".magenta().bold(), no_pods.to_string().as_str().blue());
    println!("{}", pod_viz);
    println!(
        "{} in {} as per {}",
        namespace.bold().magenta().on_black(),
        cluster_info.cluster_name.bold().magenta().on_black(),
        payload.timestamp.bold().yellow()
    );
}
fn get_pods_visualization(payload: &Payload, namespace: &str) -> (String, usize) {
    let mut pod_rows = vec![vec![
        "node".cell().bold(true),
        "pod name".cell().bold(true),
        "status".cell().bold(true),
        "tag".cell().bold(true),
    ]];

    let mut pairs: Vec<_> = payload.nodes.clone().into_iter().collect();
    pairs.sort_by_key(|pair| pair.0.clone());
    let mut total_no_pods = 0;
    for (key, value) in pairs {
        let mut pods_in_namespace: Vec<&Pods> = value
            .iter()
            .filter(|pod| pod.namespace == *namespace)
            .collect();
        total_no_pods += pods_in_namespace.len();
        pods_in_namespace.sort();
        for pod in pods_in_namespace {
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

            let pod_image_tag = pod.image.split(':').last().unwrap_or("unknown");

            pod_rows.push(vec![
                key.clone().cell(),
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
            return ("could not visualize pods".to_string(), 0);
        }
    };
    (pod_table_display.to_string(), total_no_pods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{ClusterInfo, Deployment};

    #[test]
    fn get_deployments_visualization_visualizing_only_selected_namespace() {
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

    #[test]
    fn get_endpoints_visualization_visualizing_only_selected_namespace() {
        // Arrange
        let payload = Payload {
            ingresses: Some(vec![
                models::Ingress {
                    endpoint: "endpoint1".to_string(),
                    namespace: "namespace1".to_string(),
                    ip: "172.23.1.205".to_string(),
                },
                models::Ingress {
                    endpoint: "endpoint2".to_string(),
                    namespace: "namespace2".to_string(),
                    ip: "172.23.1.205".to_string(),
                },
            ]),
            ..Default::default()
        };

        // Act
        let visualization = get_endpoints_visualization(&payload, "namespace1");

        // Assert
        // contains this
        println!("{}", visualization);

        let expected = format!(
            "{}{} ({})\n",
            "https://".bold().blue(),
            "endpoint1".bold().blue(),
            "172.23.1.205"
        );
        assert!(visualization.eq(&expected));
        // and not this
        let not_expected = format!(
            "{}{} ({})\n",
            "https://".bold().blue(),
            "endpoint2".bold().blue(),
            "172.23.1.205"
        );
        assert!(!visualization.contains(&not_expected));
    }

    #[test]
    fn get_pods_visualization_visualizing_only_selected_namespace() {
        // Arrange
        let payload = Payload {
            nodes: std::collections::HashMap::from_iter(vec![
                (
                    "node1".to_string(),
                    vec![
                        models::Pods {
                            namespace: "namespace1".to_string(),
                            name: "pod1".to_string(),
                            status: "Running".to_string(),
                            image: "image1:tag1".to_string(),
                        },
                        models::Pods {
                            namespace: "namespace2".to_string(),
                            name: "pod2".to_string(),
                            status: "Running".to_string(),
                            image: "image2:tag2".to_string(),
                        },
                    ],
                ),
                (
                    "node2".to_string(),
                    vec![models::Pods {
                        namespace: "namespace1".to_string(),
                        name: "pod3".to_string(),
                        status: "Running".to_string(),
                        image: "image3:tag3".to_string(),
                    }],
                ),
            ]),
            ..Default::default()
        };

        // Act
        let (visualization, no_of_pods) = get_pods_visualization(&payload, "namespace1");

        // Assert
        
        // no_of_pods is 2
        assert!(no_of_pods == 2);

        // contains this (pod 1 and pod 3 in node 1 and node 2)
        assert!(visualization.contains("node1"));
        assert!(visualization.contains("pod1"));
        assert!(visualization.contains("Running"));
        assert!(visualization.contains("tag1"));
        assert!(visualization.contains("node2"));
        assert!(visualization.contains("pod3"));
        assert!(visualization.contains("Running"));
        assert!(visualization.contains("tag3"));
        // and not this (pod2)
        assert!(!visualization.contains("pod2"));
        assert!(!visualization.contains("tag2"));
    }

    #[test]
    fn check_cluster_status_sets_overall_status_to_bad_if_one_pod_is_crashloopbackoff() {
        // Arrange
        let payload = Payload {
            nodes: std::collections::HashMap::from_iter(vec![(
                "node1".to_string(),
                vec![models::Pods {
                    namespace: "namespace1".to_string(),
                    name: "pod1".to_string(),
                    status: "CrashLoopBackOff".to_string(),
                    image: "image1:tag1".to_string(),
                }],
            )]),
            ..Default::default()
        };
        let host = "host1".to_string();

        // Act
        let (_, status_table) = check_cluster_status(&host, payload);

        // Assert
        assert!(status_table.contains("BAD"));
        assert!(status_table.contains("host1"));
        assert!(status_table.contains("1"));
        assert!(status_table.contains("namespace1"));
    }
    #[test]
    fn check_cluster_status_sets_overall_status_to_ok_if_all_pods_ok() {
        // Arrange
        let payload = Payload {
            nodes: std::collections::HashMap::from_iter(vec![(
                "node1".to_string(),
                vec![
                    models::Pods {
                        namespace: "namespace1".to_string(),
                        name: "pod1".to_string(),
                        status: "Running".to_string(),
                        image: "image1:tag1".to_string(),
                    },
                    models::Pods {
                        namespace: "namespace1".to_string(),
                        name: "pod2".to_string(),
                        status: "Succeeded".to_string(),
                        image: "image2:tag2".to_string(),
                    },
                    models::Pods {
                        namespace: "namespace1".to_string(),
                        name: "pod3".to_string(),
                        status: "Completed".to_string(),
                        image: "image3:tag3".to_string(),
                    },
                ],
            )]),
            ..Default::default()
        };
        let host = "host1".to_string();

        // Act
        let (_, status_table) = check_cluster_status(&host, payload);

        // Assert
        assert!(status_table.contains("OK"));
        assert!(status_table.contains("host1"));
        assert!(status_table.contains("0"));
    }
}
