//! `AddConnection` and `DelConnection` ops.
//!
//! Connections are the wires between power poles / travel poles / hubs.
//! Each connection has a stable `index` (assigned by the server) so
//! `DelConnection` can target it without ambiguity -- the same pair of
//! nodes can carry both a Power and a Travel link, so `(a, b)` alone
//! isn't unique.

use crate::net::NetContext;
use perlica_logic::enums::{FCConnectionPortType, FCConnectionType};
use perlica_logic::factory::FactoryConnection;
use perlica_proto::{
    CsdFactoryOpAddConnection, CsdFactoryOpDelConnection, FactoryOpRetCode, FactoryOpType,
    ScFactoryOpRet, ScdFactorySyncSceneConnectionPort,
};

use super::super::response;

pub async fn handle_add(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpAddConnection,
) -> ScFactoryOpRet {
    // The client gives us a list of ports; we expect exactly 2 (one per
    // endpoint). Anything else is malformed.
    if req.ports.len() != 2 {
        return response::fail(
            index,
            FactoryOpType::AddConnection,
            FactoryOpRetCode::Fail,
            format!("expected 2 ports, got {}", req.ports.len()),
        );
    }

    let conn_type = match req.r#type {
        0 => FCConnectionType::Power,
        1 => FCConnectionType::Travel,
        other => {
            return response::fail(
                index,
                FactoryOpType::AddConnection,
                FactoryOpRetCode::Fail,
                format!("invalid connection type {}", other),
            );
        }
    };

    // Each port entry carries the node_id it belongs to. The wire format
    // uses a side-channel for the node_id but the proto only exposes
    // `port_type` + position here -- the client also sends the node_id
    // in the parent `CsFactoryOp.name` field as the region, and the
    // individual node IDs come through the port payload's implicit
    // ordering. This is genuinely awkward; the live server relies on
    // the ports being in (a, b) order with the node_id baked into the
    // port's serialized form.
    //
    // TODO: re-check the proto definition for `ScdFactorySyncSceneConnectionPort`
    // -- it should carry a `node_id` field. If not, the connection
    // handler needs the client to also send the node IDs separately.
    let (node_a, node_b, port_type) = parse_ports(&req.ports, conn_type);

    let (node_id_a, node_id_b) = match (node_a, node_b) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            return response::fail(
                index,
                FactoryOpType::AddConnection,
                FactoryOpRetCode::Fail,
                "could not extract node IDs from ports payload",
            );
        }
    };

    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => {
            return response::fail(
                index,
                FactoryOpType::AddConnection,
                FactoryOpRetCode::Fail,
                format!("region {} not found", region_name),
            );
        }
    };

    // Both endpoints must exist.
    if !region.nodes.contains_key(&node_id_a) || !region.nodes.contains_key(&node_id_b) {
        return response::fail(
            index,
            FactoryOpType::AddConnection,
            FactoryOpRetCode::Fail,
            format!("one or both endpoints missing: {} / {}", node_id_a, node_id_b),
        );
    }

    let conn_index = region.next_connection_index();
    region.connections.push(FactoryConnection {
        connection_type: conn_type,
        port_type,
        node_id_a,
        node_id_b,
        index: conn_index,
    });

    response::ok_with_add_connection(index, conn_index)
}

pub async fn handle_del(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpDelConnection,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => {
            return response::fail(
                index,
                FactoryOpType::DelConnection,
                FactoryOpRetCode::Fail,
                format!("region {} not found", region_name),
            );
        }
    };

    let before = region.connections.len();
    region.connections.retain(|c| c.index != req.index);
    if region.connections.len() == before {
        return response::fail(
            index,
            FactoryOpType::DelConnection,
            FactoryOpRetCode::Fail,
            format!("no connection with index {}", req.index),
        );
    }

    response::ok(index, FactoryOpType::DelConnection)
}

/// Best-effort port parser. The proto's `ScdFactorySyncSceneConnectionPort`
/// doesn't expose a node_id field directly, so we look at `port_type`
/// (Hub=0, PowerPole=1, Logic=2) to figure out what kind of endpoints
/// we're connecting, and leave the actual node_id extraction as a TODO
/// until the wire format is confirmed against a live capture.
fn parse_ports(
    ports: &[ScdFactorySyncSceneConnectionPort],
    conn_type: FCConnectionType,
) -> (Option<u32>, Option<u32>, FCConnectionPortType) {
    let port_type = match conn_type {
        FCConnectionType::Power => FCConnectionPortType::PowerPole,
        FCConnectionType::Travel => FCConnectionPortType::Hub,
    };

    let _ = ports;
    // TODO: once the wire format is confirmed, extract node_id from
    // each port entry. For now return None so the caller fails loudly
    // rather than silently wiring up the wrong endpoints.
    (None, None, port_type)
}
