import { join } from '@std/path';
import * as uuidLib from '@std/uuid';

import { Hono } from 'hono';
import { showRoutes } from 'hono/dev';
import { cors } from 'hono/cors';
import { upgradeWebSocket } from 'hono/deno';
import { type WSContext } from 'hono/ws';

import { publicIpv4 } from 'npm:public-ip@7.0.1';

import { ArmaReforgerServer } from './ars.ts';
import { createSharedFolders, directoryExists, fileExists } from './utils.ts';
import { getServer, getServers } from './servers.ts';
import { getLogFile, getLogs, isValidLogDirName } from './logs.ts';
import type { Server, ServerConfig } from './interfaces.ts';
import {
	addToKnownPlayers,
	getKnownPlayers,
	getPlayersFromLog,
} from './players.ts';

if (import.meta.main) {
	// create needed folders if in development environment
	const environment = Deno.env.get('ENVIRONMENT') || 'Production';
	if (environment === 'Development') {
		await createSharedFolders();
	}

	// INIT: read all existing server.json and create ars instances
	const arsList: ArmaReforgerServer[] = [];
	const dir = join(Deno.cwd(), 'servers');

	try {
		for await (const dirEntry of Deno.readDir(dir)) {
			const fileContent = await Deno.readTextFile(
				join(dir, dirEntry.name, 'server.json'),
			);
			const server = JSON.parse(fileContent) as Server;
			arsList.push(new ArmaReforgerServer(server.uuid));
			console.log(
				`INIT: Adding Arma Reforger Server with UUID: ${server.uuid}`,
			);
		}
	} catch (error) {
		console.log(error);
	}

	const publicIp = await publicIpv4();

	/* ---------------------------------------- */

	// INIT: configure and start hono server with routes and websocket
	const app = new Hono();
	app.use('/api/*', cors());

	/* ---------------------------------------- */

	// route for adding a new server incl. it's config
	app.post('/api/add-server', async (c) => {
		const server: Server = await c.req.json();
		const uuid = crypto.randomUUID();
		server.uuid = uuid;
		const config = server.config;
		delete server.config;

		const serversDir = join(Deno.cwd(), 'servers', uuid);
		if (await directoryExists(serversDir)) return c.json({}, 500);

		const serverPath = join(serversDir, 'server.json');
		const configPath = join(serversDir, 'config.json');

		try {
			await Deno.mkdir(serversDir);
			await Deno.writeTextFile(
				serverPath,
				JSON.stringify(server, null, 2),
			);
			await Deno.writeTextFile(
				configPath,
				JSON.stringify(config, null, 2),
			);
		} catch (error) {
			console.log(error);
		}

		arsList.push(new ArmaReforgerServer(uuid));
		console.log(`Added Arma Reforger Server with UUID: ${uuid}`);
		return c.json({ uuid });
	});

	/* ---------------------------------------- */

	// route for updating an existing server incl. it's config
	app.put('/api/server/:uuid', async (c) => {
		const uuid = c.req.param('uuid');
		if (!uuidLib.validate(uuid)) return c.json({ value: false }, 404);

		const serversDir = join(Deno.cwd(), 'servers', uuid);
		if (!await directoryExists(serversDir)) return c.json({ value: false }, 404);

		const server: Server = await c.req.json();
		if (!server.config) return c.json({ value: false }, 500);

		const config: ServerConfig = server.config;
		delete server.config;

		const serverPath = join(serversDir, 'server.json');
		const configPath = join(serversDir, 'config.json');
		Deno.writeTextFile(serverPath, JSON.stringify(server, null, 2));
		Deno.writeTextFile(configPath, JSON.stringify(config, null, 2));

		console.log(
			`Updated Arma Reforger Server with UUID: ${uuid}`,
		);
		return c.json({ value: true });
	});

	/* ---------------------------------------- */

	// route for getting names of existing log names (containing dates)
	app.get('/api/server/:uuid/logs', async (c) => {
		const uuid = c.req.param('uuid');
		if (!uuidLib.validate(uuid)) return c.json([], 404);

		const logs = await getLogs(uuid);

		console.log(
			`Getting Log Events for Arma Reforger Server with UUID: ${uuid}`,
		);
		return c.json(logs);
	});

	/* ---------------------------------------- */

	// route for getting a specific log file
	app.get('/api/server/:uuid/log/:log/:file', async (c) => {
		const { uuid, log, file } = c.req.param();
		if (!uuidLib.validate(uuid)) return c.text('', 404);
		if (!isValidLogDirName(log)) return c.text('', 404);
		if (!['console.log', 'error.log', 'script.log'].includes(file)) {
			return c.text('', 404);
		}

		console.log(
			`Getting Log File ${log}/${file} for Arma Reforger Server with UUID: ${uuid}`,
		);
		const logFile = await getLogFile(uuid, log, file);
		return c.text(logFile);
	});

	/* ---------------------------------------- */

	// route for getting all players from a specific log file
	app.get('/api/server/:uuid/log-players/:log', async (c) => {
		const { uuid, log } = c.req.param();
		if (!uuidLib.validate(uuid)) return c.json([], 404);
		if (!isValidLogDirName(log)) return c.json([], 404);

		const logPath = join(
			Deno.cwd(),
			'profiles',
			uuid,
			'logs',
			log,
			'console.log',
		);
		if (!await fileExists(logPath)) return c.json([], 404);

		console.log(
			`Getting Players from Log ${log} for Arma Reforger Server with UUID: ${uuid}`,
		);

		const newPlayers = await getPlayersFromLog(logPath);
		await addToKnownPlayers(uuid, newPlayers);

		return c.json(newPlayers);
	});

	/* ---------------------------------------- */

	// route for getting all known players
	app.get('/api/server/:uuid/known-players', async (c) => {
		const { uuid } = c.req.param();
		if (!uuidLib.validate(uuid)) return c.json([], 404);

		console.log(
			`Getting known Players for Arma Reforger Server with UUID: ${uuid}`,
		);

		const knownPlayers = await getKnownPlayers(uuid);

		return c.json(knownPlayers);
	});

	/* ---------------------------------------- */

	// route for getting the public ip of the host
	app.get('/api/get-public-ip', (c) => {
		console.log(`Getting Public IP of this Host: ${publicIp}`);
		return c.json({ ipv4: publicIp });
	});

	/* ---------------------------------------- */

	// route for getting all servers and their configs
	app.get('/api/get-servers', async (c) => {
		console.log(`Getting list of Arma Reforger Servers.`);
		
		const servers: Server[] = await getServers();
		return c.json(servers);
	});

	/* ---------------------------------------- */

	// route for getting a specific server
	app.get('/api/server/:uuid', async (c) => {
		const uuid = c.req.param('uuid');
		if (!uuidLib.validate(uuid)) return c.json({}, 404);

		console.log(
			`Getting Arma Reforger Server with UUID: ${uuid}`,
		);

		const server = await getServer(uuid);
		if (server === null) { c.json({}), 404 }
		return c.json(server);
	});

	/* ---------------------------------------- */

	// route for starting a specific server
	app.get('/api/server/:uuid/start', (c) => {
		const uuid = c.req.param('uuid');
		if (!uuidLib.validate(uuid)) return c.json({value: false}, 404);

		console.log(
			`Starting Arma Reforger Server with UUID: ${uuid}`,
		);

		// starting ars instance
		const ars = arsList.find((i) => i.uuid === uuid);
		if (ars) {
			ars.start();
			return c.json({ value: true });
		} else {
			console.log(`Arma Reforger Server with UUID ${uuid} not found.`);
			return c.json({ value: false }, 404);
		}
	});

	/* ---------------------------------------- */

	// route for stopping a specific server
	app.get('/api/server/:uuid/stop', (c) => {
		const uuid = c.req.param('uuid');
		if (!uuidLib.validate(uuid)) return c.json({value: false}, 404);

		console.log(
			`Stopping Arma Reforger Server with UUID: ${uuid}`,
		);

		// stop running ars instance
		const ars = arsList.find((i) => i.uuid === uuid);
		if (ars) {
			ars.stop();
			return c.json({ value: true });
		} else {
			console.log(`Arma Reforger Server with UUID ${uuid} not found.`);
			return c.json({ value: false }, 404);
		}
	});

	/* ---------------------------------------- */

	// route for deleting a specific server
	app.delete('/api/server/:uuid', async (c) => {
		const uuid = c.req.param('uuid');
		if (!uuidLib.validate(uuid)) return c.json({value: false}, 404);

		console.log(
			`Deleting Arma Reforger Server with UUID: ${uuid}`,
		);

		// delete server and profile
		const profilePath = join(Deno.cwd(), 'profiles', uuid);
		try {
			await Deno.remove(profilePath, { recursive: true });
		} catch (error) {
			if (!(error instanceof Deno.errors.NotFound)) {
				throw error;
			}
		}
		const serverPath = join(Deno.cwd(), 'servers', uuid);
		try {
			await Deno.remove(serverPath, { recursive: true });
		} catch (error) {
			if (!(error instanceof Deno.errors.NotFound)) {
				throw error;
			}
		}
		const ars = arsList.find((i) => i.uuid === uuid);
		if (ars) {
			arsList.splice(arsList.indexOf(ars), 1);
			return c.json({ value: true });
		} else {
			console.log(`Arma Reforger Server with UUID ${uuid} not found.`);
			return c.json({ value: false }, 404);
		}
	});

	/* ---------------------------------------- */

	// route for starting the isRunning state of a specific server
	app.get('/api/server/:uuid/isRunning', (c) => {
		const uuid = c.req.param('uuid');
		if (!uuidLib.validate(uuid)) return c.json({value: false}, 404);

		const ars = arsList.find((i) => i.uuid === uuid);
		if (ars) {
			console.log(
				`Arma Reforger Server with UUID ${uuid} is running: ${ars.isRunning}`,
			);
			return c.json({ value: ars.isRunning });
		} else {
			console.log(`Arma Reforger Server with UUID ${uuid} not found.`);
			return c.json({ value: false }, 404);
		}
	});

	/* ---------------------------------------- */

	const wsClients: WSContext[] = [];

	// websocket configuration for server to client push notifications
	app.get(
		'/ws',
		upgradeWebSocket((c) => {
			return {
				onMessage(event, ws) {
					if (event.data === 'ping') {
						ws.send('pong');
					} else {
						console.log(`Unknown WebSocket message: ${event.data}`);
					}
				},
				onOpen: (event, ws) => {
					wsClients.push(ws);
					console.log('WebSocket opened.');
				},
				onClose: () => {
					console.log('WebSocket closed.');
				},
			};
		}),
	);

	/* ---------------------------------------- */

	const server = Deno.serve({ port: 3_000 }, app.fetch);

	showRoutes(app);

	/* ---------------------------------------- */

	/* 	WebSocket.CONNECTING (0)
	Socket has been created. The connection is not yet open.

	WebSocket.OPEN (1)
	The connection is open and ready to communicate.

	WebSocket.CLOSING (2)
	The connection is in the process of closing.

	WebSocket.CLOSED (3)
	The connection is closed or couldn't be opened. */

	// broadcast all updates related to ars isRunning status to all clients
	const interval = setInterval(() => {
		arsList.forEach((ars) => {
			while (ars.messageQueue.length > 0) {
				const message = ars.messageQueue.splice(0, 1)[0];
				wsClients.forEach((wsClient, index, object) => {
					while (wsClient.readyState === 0) {
						// itentionally do nothing
					}
					if (wsClient.readyState === 1) {
						wsClient.send(JSON.stringify(message));
					} else {
						wsClient.close();
						object.splice(index, 1); // iterate and mutate
						console.log('Closed non ready websocket.');
					}
				});
				console.log(
					`Sent message to all clients: ${JSON.stringify(message)}`,
				);
			}
		});
	}, 3_000);

	/* ---------------------------------------- */

	// things to do if <CTRL+C> is pressed
	Deno.addSignalListener(
		'SIGINT',
		async () => {
			console.log('SIGINT!\n');
			await server.shutdown();
			Deno.exit(0);
		},
	);
}
