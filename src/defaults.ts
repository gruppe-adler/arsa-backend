export const defaultConfig = {
    bindAddress: '0.0.0.0',
    bindPort: 2001,
    publicAddress: '',
    publicPort: 2001,
    a2s: {
        address: '0.0.0.0',
        port: 17777
    },
    rcon: {
        address: '0.0.0.0',
        port: 19999,
        password: 'mE*mueuJnG27@LZrgK4pLjEL',
        maxClients: 16,
        permission: 'monitor',
        blacklist: [],
        whitelist: []
    },
    game: {
        name: 'server name',
        password: 'bEBArjt_Jt-A!Ce*xrr2huQo',
        passwordAdmin: 'gHuaVyZCFQ!Rg984HMj6aoHn',
        admins: [],
        scenarioId: '{59AD59368755F41A}Missions/21_GM_Eden.conf',
        maxPlayers: 32,
        visible: true,
        crossPlatform: false,
        supportedPlatforms: [
            "PLATFORM_PC",
            "PLATFORM_XBL"
        ],
        gameProperties: {
            serverMaxViewDistance: 2500,
            serverMinGrassDistance: 50,
            fastValidation: true,
            networkViewDistance: 1000,
            battlEye: true,
            disableThirdPerson: true,
            VONDisableUI: true,
            VONDisableDirectSpeechUI: true,
            VONCanTransmitCrossFaction: false,
            missionHeader: {}
        },
        mods: []
    },
    operating: {
        lobbyPlayerSynchronise: true,
        disableNavmeshStreaming: undefined,
        disableServerShutdown: false,
        disableAI: false,
        playerSaveTime: 120,
        aiLimit: -1,
        slotReservationTimeout: 60
    }
}

export const defaultServer = {
    uuid: '',
    name: '',
    config: defaultConfig
}