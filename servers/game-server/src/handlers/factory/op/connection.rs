use crate::net::NetContext;
use perlica_logic::enums::FCConnectionType;
use perlica_proto::{
    CsdFactoryOpAddConnection, CsdFactoryOpDelConnection, FactoryOpRetCode, FactoryOpType,
    ScFactoryOpRet,
};

use super::super::response;

pub async fn handle_add(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpAddConnection,
) -> ScFactoryOpRet {
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
                format!("invalid connection type {other}"),
            );
        }
    };

    // TODO: extract node IDs from the port payload once the wire format
    // is confirmed. For now we can't add connections without node IDs.
    let _ = (conn_type, ctx, region_name);

    response::fail(
        index,
        FactoryOpType::AddConnection,
        FactoryOpRetCode::Fail,
        "could not extract node IDs from ports payload",
    )
}

pub async fn handle_del(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpDelConnection,
) -> ScFactoryOpRet {
    match ctx.player.factory.del_connection(&region_name, req.index) {
        Ok(()) => response::ok(index, FactoryOpType::DelConnection),
        Err(msg) => response::fail(
            index,
            FactoryOpType::DelConnection,
            FactoryOpRetCode::Fail,
            msg,
        ),
    }
}
