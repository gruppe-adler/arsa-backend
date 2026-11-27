use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Lowest game port a group may start at. ARS containers have no capabilities, so they can't
/// bind privileged ports.
pub const MIN_GAME_PORT: u16 = 1024;
/// The group spans `game_port..=game_port + 2`, so it has to start low enough to fit.
pub const MAX_GAME_PORT: u16 = u16::MAX - 2;

/// A predefined set of ports for one server: game port, RCON port (game + 1) and A2S port (game + 2).
#[sea_orm::model]
#[derive(Debug, Clone, PartialEq, Eq, DeriveEntityModel, ToSchema, Deserialize, Serialize)]
#[sea_orm(table_name = "port_group")]
#[serde(rename_all = "camelCase")]
#[schema(as = PortGroup)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    /// Game (bind and public) port. RCON uses `gamePort + 1`, A2S uses `gamePort + 2`.
    pub game_port: u16,
}

impl Model {
    pub fn rcon_port(&self) -> u16 {
        self.game_port + 1
    }

    pub fn a2s_port(&self) -> u16 {
        self.game_port + 2
    }

    pub fn ports(&self) -> [u16; 3] {
        [self.game_port, self.rcon_port(), self.a2s_port()]
    }

    /// Two groups overlap when any of their three ports are the same.
    pub fn overlaps(&self, game_port: u16) -> bool {
        self.game_port.abs_diff(game_port) < 3
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortGroupRequest {
    pub name: String,
    /// Game port. RCON uses `gamePort + 1`, A2S uses `gamePort + 2`.
    pub game_port: u16,
}

#[cfg(test)]
mod tests {
    use super::Model;

    fn group(game_port: u16) -> Model {
        Model {
            id: 1,
            name: "A".to_string(),
            game_port,
        }
    }

    #[test]
    fn ports_follow_game_port() {
        assert_eq!(group(2001).ports(), [2001, 2002, 2003]);
    }

    #[test]
    fn overlaps_only_when_ports_are_shared() {
        let g = group(2001);
        assert!(g.overlaps(1999));
        assert!(g.overlaps(2001));
        assert!(g.overlaps(2003));
        assert!(!g.overlaps(1998));
        assert!(!g.overlaps(2004));
    }
}
