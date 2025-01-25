export interface ServerStatusUpdate {
	type: 'isRunningUpdate' | 'arsStatusUpdate' | 'message';
}

export interface IsRunningUpdate extends ServerStatusUpdate {
	type: 'isRunningUpdate';
	uuid: string;
	isRunning: boolean;
}

export interface ArsStatusUpdate extends ServerStatusUpdate {
	type: 'arsStatusUpdate';
	arsStatus: ArsStatus;
}

export interface ServerMessage extends ServerStatusUpdate {
	type: 'message';
	message: string;
}

export interface Server {
	uuid: string;
	name: string;
	isRunning: boolean;
	config?: ServerConfig;
	startupParameters: StartupParameter[];
}

export interface ServerConfig {
	bindAddress: string;
	bindPort: number;
	publicAddress: string;
	publicPort: number;
	a2s: {
		address: string;
		port: number;
	};
	rcon: {
		address: string;
		port: number;
		password: string;
		maxClients: number;
		permission: string;
		blacklist: string[];
		whitelist: string[];
	};
	game: {
		name: string;
		password: string;
		passwordAdmin: string;
		admins: string[];
		scenarioId: string;
		maxPlayers: number;
		visible: boolean;
		crossPlatform: boolean;
		supportedPlatforms: Platform[];
		gameProperties: {
			serverMaxViewDistance: number;
			serverMinGrassDistance: number;
			fastValidation: boolean;
			networkViewDistance: number;
			battlEye: boolean;
			disableThirdPerson: boolean;
			VONDisableUI: boolean;
			VONDisableDirectSpeechUI: boolean;
			VONCanTransmitCrossFaction: boolean;
			missionHeader: object;
		};
		modsRequiredByDefault: boolean;
		mods: Mod[];
	};
	operating: {
		lobbyPlayerSynchronise: boolean;
		disableCrashReporter: boolean;
		disableNavmeshStreaming: string[] | undefined;
		disableServerShutdown: boolean;
		disableAI: boolean;
		playerSaveTime: number;
		aiLimit: number;
		slotReservationTimeout: number;
		joinQueue: {
			maxSize: number;
		};
	};
}

export type Platform = 'PLATFORM_PC' | 'PLATFORM_XBL' | 'PLATFORM_PSN';

export interface Mod {
	modId: string;
	name: string;
	version: string | undefined;
	required: boolean | undefined;
}

export interface PlayerIdentityId {
	name: string;
	identityId: string;
}

export interface StartupParameter {
	parameter: string;
	tooltip: string;
	enabled: boolean;
	type: 'number' | 'string' | 'select';
	value: number | string | undefined;
	valueList?: string[];
	minVal?: number;
	maxVal?: number;
}

export interface DockerStats {
	BlockIO: string;
	CPUPerc: string;
	Container: string;
	ID: string;
	MemPerc: string;
	MemUsage: string;
	Name: string;
	NetIO: string;
	PIDs: string;
}

export enum ArsStatus {
	UNKNOWN,
	AVAILABLE,
	RECREATING,
	RECREATING_FAILURE,
	UNAVAILABLE,
}

export interface DockerInspect {
	Id: string;
	RepoTags: string[];
	Parent: string;
	Comment: string;
	Created: string;
	DockerVersion: string;
	Author: string;
	Config: DockerConfig;
	Architecture: string;
	Os: string;
	Size: number;
	GraphDriver: {
		Data: {
			LowerDir: string;
			MergedDir: string;
			UpperDir: string;
			WorkDir: string;
		};
		Name: string;
	};
	RootFS: {
		Type: string;
		Layers: string[];
	};
	Metadata: {
		LastTagTime: string;
	};
}

export interface DockerConfig {
	Hostname: string;
	Domainname: string;
	User: string;
	AttachStdin: boolean;
	AttachStdout: boolean;
	AttachStderr: boolean;
	ExposedPorts: unknown; // ???
	Tty: boolean;
	OpenStdin: boolean;
	StdinOnce: boolean;
	Env: string[];
	Cmd: null; // ???
	ArgsEscaped: boolean;
	Image: string;
	Volumes: null; // ???
	WorkingDir: string;
	Entrypoint: string[];
	OnBuild: null; // ???
	Labels: {
		'org.opencontainers.image.ref.name': string;
		'org.opencontainers.image.version': string;
	};
	Shell: string[];
}
