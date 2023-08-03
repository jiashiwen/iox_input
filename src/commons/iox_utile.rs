use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use anyhow::Ok;
use anyhow::Result;
use futures::stream;
use influxdb_iox_client::{connection::Builder, write::Client};
use serde::{Deserialize, Serialize};
use serde_yaml::from_str;

use super::json_utile::json_to_struct;

pub const SCHEMA_STRING: &'static str = "schema:
- measurement: container_disk
  metrics:
    - container_disk_io_util
    - container_disk_read_iops
    - container_disk_reads_rate
    - container_disk_write_iops
    - container_disk_writes_rate
- measurement: container_fs
  metrics:
    - container_fs_inodes_free
    - container_fs_inodes_free_ratio
    - container_fs_inodes_total
    - container_fs_inodes_used_ratio
    - container_fs_limit
    - container_fs_usage
    - container_fs_usage_ratio
- measurement: pod_cpu_mem
  metrics:
    - pod_cpu_request_utilisation
    - pod_cpu_request_utilisation_max
    - pod_cpu_system_utilisation
    - pod_cpu_user_utilisation
    - pod_cpu_utilisation
    - pod_cpu_utilisation_max
    - pod_memory_cache
    - pod_memory_rss
    - pod_memory_rss_ratio
    - pod_memory_swap
    - pod_memory_swap_ratio
    - pod_spec_memory_limit
- measurement: pod_tcp_ip
  metrics:
    - pod_ip_in_discards
    - pod_ip_out_discards
    - pod_ip_in_receives # in pps
    - pod_ip_out_requests # out pps
    - pod_tcp_close_wait
    - pod_tcp_estab
    - pod_tcp_fin_wait1
    - pod_tcp_fin_wait2
    - pod_tcp_in_errs
    - pod_tcp_in_segs
    - pod_tcp_listen
    - pod_tcp_out_segs
    - pod_tcp_retrans_segs
    - pod_tcp_time_wait
- measurement: pod_iface
  metrics:
    - pod_net_received_drop_packets_rate
    - pod_net_received_rate
    - pod_net_transmitted_drop_packets_rate
    - pod_net_transmitted_rate
- measurement: pod_load
  metrics:
    - pod_load1
    - pod_load5
    - pod_load15
- measurement: proc
  metrics:
    - proc_cpu_usage
    - proc_fd_num
    - proc_mem_usage
    - proc_num
    - proc_status
- measurement: app_meta
  metrics:
    - app_kube_pod_info
- measurement: node_cpu_mem
  metrics:
    - node_cpu_cores
    - node_cpu_iowait_utilisation
    - node_cpu_system_utilisation
    - node_cpu_user_utilisation
    - node_cpu_utilisation
    - node_memory_rss
    - node_memory_rss_utilisation
    - node_memory_swap_ratio
    - node_memory_total
    - node_memory_utilisation
- measurement: node_disk
  metrics:
    - node_disk_io_util
    - node_disk_read_iops
    - node_disk_read_rate
    - node_disk_write_iops
    - node_disk_write_rate
- measurement: node_filesystem
  metrics:
    - node_filesystem_available
    - node_filesystem_inode_usage_ratio
    - node_filesystem_usage_ratio
- measurement: node_net
  metrics:
    - node_network_received_drop_packets_rate
    - node_network_receive_packets_rate
    - node_network_receive_rate
    - node_network_transmitted_drop_packets_rate
    - node_network_transmit_packets_rate
    - node_network_transmitted_rate";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Meta {
    pub name: String,
    pub uuid: String,
    pub tags: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Metric {
    pub meta: Meta,
    pub time: Vec<u128>,
    pub value: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Schemas {
    pub schema: Vec<schema>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct schema {
    pub measurement: String,
    pub metrics: Vec<String>,
}

pub async fn gen_iox_client(server_address: &str) -> Result<Client> {
    let connection = Builder::default().build(server_address).await?;
    // let namespace = String::from(namespace);
    let iox_client = influxdb_iox_client::write::Client::new(connection.clone());
    Ok(iox_client)
}

pub fn get_schema_map() -> Result<HashMap<String, String>> {
    let schemas = from_str::<Schemas>(SCHEMA_STRING)?;
    let mut schema_map: HashMap<String, String> = HashMap::new();

    for schema in schemas.schema {
        for metric in schema.metrics {
            schema_map.insert(metric, schema.measurement.clone());
        }
    }
    Ok(schema_map)
}

pub fn format_to_lp(content: &str) -> Result<Vec<String>> {
    let schema_map = get_schema_map()?;
    let metric = json_to_struct::<Metric>(content)?;

    let mut vec_lp = vec![];

    let tags = metric.meta.tags;
    let mut tags_str = tags
        .iter()
        // .map(|v| v[0].clone() + "=" + "\"" + &v[1] + "\"" + ",")
        .map(|v| v[0].clone() + "=" + &v[1].replace(" ", "\\ ") + ",")
        .collect::<String>();
    tags_str.pop();

    for (idx, time) in metric.time.iter().enumerate() {
        // line protocl 格式
        // cpu,host="192.168.1.1",region="cn-north-1",key=a\ b idle=0.33,user=0.16 1556813561098000000
        // <measurement>[,<tag_key>=<tag_value>[,<tag_key>=<tag_value>]] <field_key>=<field_value>[,<field_key>=<field_value>] [<timestamp>]
        let mut field = metric.meta.name.clone();
        let measurement = match schema_map.get(field.as_str()) {
            Some(s) => s,
            None => {
                continue;
            }
        };
        field.push_str("=");
        field.push_str(metric.value[idx].to_string().as_str());

        let timestemp = time * 1000000;
        let lp = format!("{},{} {} {}", measurement, tags_str, field, timestemp);
        vec_lp.push(lp);
    }
    Ok(vec_lp)
}

pub async fn write_json_to_iox(
    server: &str,
    namespace: &str,
    vec_json: Vec<String>,
    lp_count: Arc<AtomicUsize>,
) -> Result<()> {
    let mut count_lp: usize = 0;
    let mut iox_client = gen_iox_client(&server).await?;

    for json in vec_json {
        let vec_lp = match format_to_lp(json.as_str()) {
            anyhow::Result::Ok(v) => {
                count_lp += 1;
                v
            }
            Err(e) => {
                log::error!("{}", e);
                continue;
            }
        };
        let stream = stream::iter(vec_lp);
        if let Err(e) = iox_client.write_lp_stream(namespace, stream).await {
            log::error!("{}", e);
            continue;
        };
        lp_count.fetch_add(count_lp, std::sync::atomic::Ordering::SeqCst);
    }
    log::info!("lp count:{:?}", lp_count);

    Ok(())
}
