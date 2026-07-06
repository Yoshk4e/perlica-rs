use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{
    CsdFactoryOpSetTravelPoleDefaultNext, CsdFactoryOpUseHealTowerPoint, FactoryOpRetCode,
    FactoryOpType, ScFactoryOpRet,
};

use super::super::response;

pub async fn handle_use_heal_tower(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpUseHealTowerPoint,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .use_heal_tower(&region_name, req.component_id, req.use_count)
    {
        Ok(granted) => response::ok_with_use_heal_tower(index, granted),
        Err(e) => response::fail(
            index,
            FactoryOpType::UseHealTowerPoint,
            FactoryOpRetCode::Fail,
            match e {
                ops::TargetError::RegionNotFound => format!("region {region_name} not found"),
                ops::TargetError::ComponentNotFound => {
                    format!("component {} not found", req.component_id)
                }
                ops::TargetError::WrongComponentType => {
                    format!("component {} is not a HealTower", req.component_id)
                }
            },
        ),
    }
}

pub async fn handle_set_travel_pole_next(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpSetTravelPoleDefaultNext,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .set_travel_pole_next(&region_name, req.component_id, req.default_next)
    {
        Ok(()) => response::ok(index, FactoryOpType::SetTravelPoleDefaultNext),
        Err(e) => response::fail(
            index,
            FactoryOpType::SetTravelPoleDefaultNext,
            FactoryOpRetCode::Fail,
            match e {
                ops::TargetError::RegionNotFound => format!("region {region_name} not found"),
                ops::TargetError::ComponentNotFound => {
                    format!("target node {} not found", req.default_next)
                }
                ops::TargetError::WrongComponentType => {
                    format!("component {} is not a TravelPole", req.component_id)
                }
            },
        ),
    }
}
