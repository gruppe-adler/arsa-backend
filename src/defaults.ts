import type { Server, ServerConfig, StartupParameter } from './interfaces.ts';

export const defaultConfig: ServerConfig = {
	bindAddress: '0.0.0.0',
	bindPort: 2001,
	publicAddress: '',
	publicPort: 2001,
	a2s: {
		address: '0.0.0.0',
		port: 17777,
	},
	rcon: {
		address: '0.0.0.0',
		port: 19999,
		password: 'mE*mueuJnG27@LZrgK4pLjEL',
		maxClients: 16,
		permission: 'monitor',
		blacklist: [],
		whitelist: [],
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
			'PLATFORM_PC',
			'PLATFORM_XBL',
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
			missionHeader: {},
		},
		mods: [],
	},
	operating: {
		lobbyPlayerSynchronise: true,
		disableNavmeshStreaming: undefined,
		disableServerShutdown: false,
		disableAI: false,
		playerSaveTime: 120,
		aiLimit: -1,
		slotReservationTimeout: 60,
	},
};

export const StartupParameters: StartupParameter[] = [
	{
		parameter: 'autoReload',
		tooltip: 'value is in seconds',
		enabled: false,
		type: 'number',
		value: 10,
		minVal: 0,
	},
	{
		parameter: 'loadSessionSave',
		tooltip:
			'It can be used alone to load the latest save, or with a specific save file name.',
		enabled: false,
		type: 'string',
		value: '',
	},
	{
		parameter: 'logStats',
		tooltip: 'defined interval in milliseconds',
		enabled: false,
		type: 'number',
		value: 10000,
		minVal: 1,
	},
	{
		parameter: 'maxFPS',
		tooltip: 'should always be set to prevent high load on server',
		enabled: true,
		type: 'number',
		value: 30,
		minVal: 1,
	},
	{
		parameter: 'nds',
		tooltip:
			'The provided value stands for diameter, or the number of cells which are being replicated - default is 2 in each direction.',
		enabled: false,
		type: 'number',
		value: 2,
		minVal: 1,
	},
	{
		parameter: 'nwkResolution',
		tooltip:
			'defines what resolution Spatial Map cells should be set at in a 100..1000m range',
		enabled: false,
		type: 'number',
		value: 500,
		minVal: 100,
		maxVal: 1000,
	},
	{
		parameter: 'rpl-timeout-ms',
		tooltip: "sets the client/server timeout's value, in milliseconds",
		enabled: false,
		type: 'number',
		value: 10000,
		minVal: 1,
	},
	{
		parameter: 'staggeringBudget',
		tooltip:
			'defines how many stationary spatial map cells are allowed to be processed in one tick in 1..10201 range',
		enabled: false,
		type: 'number',
		value: 5000,
		minVal: 1,
		maxVal: 10201,
	},
	{
		parameter: 'streamingBudget',
		tooltip:
			'The global streaming budget that is equally distributed between all connections. It cannot go under 100 to prevent the system stalling.',
		enabled: false,
		type: 'number',
		value: 500,
		minVal: 100,
	},
	{
		parameter: 'streamsDelta',
		tooltip:
			'is a tool to limit the amount of streams being opened for a client in range 1..1000 (default 100)',
		enabled: false,
		type: 'number',
		value: 200,
		minVal: 1,
		maxVal: 1000,
	},
	{
		parameter: 'keepNumOfLogs',
		tooltip: 'sets the maximum amount of logs to keep (default: 10)',
		enabled: false,
		type: 'number',
		value: 10,
		minVal: 1,
	},
	{
		parameter: 'logLevel',
		tooltip:
			'allows for different log levels. Each level includes the ones below it (e.g error includes error and fatal). Possible values range from normal (where everything is logged) to fatal (where only extreme issues are logged)',
		enabled: false,
		type: 'select',
		value: 'normal',
		valueList: ['normal', 'warning', 'error', 'fatal'],
	},
];

export const defaultServer: Server = {
	uuid: '',
	name: '',
	isRunning: false,
	config: defaultConfig,
	startupParameters: StartupParameters,
};
