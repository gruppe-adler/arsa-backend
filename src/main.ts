import '@std/dotenv/load'; // Automatically load environment variables from a `.env` file
import { join } from '@std/path';

// @deno-types="npm:@types/express@4.17.15"
import express from 'npm:express@^4.17.15';
// @deno-types="npm:@types/express-ws@^3.0.5"
import expressWs from 'npm:express-ws@^5.0.2';
import cors from 'npm:cors@2.8.5';
import { publicIpv4 } from 'npm:public-ip@7.0.1';

import { ArmaReforgerServer } from './ars.ts';
import { getLogFile, getLogs, getServer, getServers } from './utils.ts';
import type { Server } from './interfaces.ts';

if (import.meta.main) {
	// INIT: read all existing server.json and create ars instances
	const arsList: ArmaReforgerServer[] = [];
	const dir = join(Deno.cwd(), 'servers');
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

	/* ---------------------------------------- */

	// INIT: configure and start express server with routes and websocket
	const app = express();
	const wsInstance = expressWs(app);
	app.use(cors());
	app.use(express.json()); // parsing JSON in req; result available in req.json

	/* ---------------------------------------- */

	// route for adding a new server incl. it's config
	app.post('/api/add-server/', (req, res) => {
		const server = req.body as Server;
		const uuid = crypto.randomUUID();
		server.uuid = uuid;
		const config = server.config;
		delete server.config;
		Deno.mkdir(`servers/${uuid}`)
			.then(() => {
				return Deno.writeTextFile(
					`./servers/${uuid}/server.json`,
					JSON.stringify(server, null, 2),
				);
			})
			.then(() => {
				return Deno.writeTextFile(
					`./servers/${uuid}/config.json`,
					JSON.stringify(config, null, 2),
				);
			});

		arsList.push(new ArmaReforgerServer(uuid));
		console.log(`Added Arma Reforger Server with UUID: ${uuid}`);
		res.json({ uuid });
	});

	/* ---------------------------------------- */

	// route for updating an existing server incl. it's config
	app.post('/api/server/:uuid/update', (req, res) => {
		const server = req.body as Server;
		const config = server.config;
		delete server.config;
		Deno.writeTextFile(
			`./servers/${req.params.uuid}/server.json`,
			JSON.stringify(server, null, 2),
		);
		Deno.writeTextFile(
			`./servers/${req.params.uuid}/config.json`,
			JSON.stringify(config, null, 2),
		);

		console.log(
			`Updated Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		res.json({ value: true });
	});

	/* ---------------------------------------- */

	// route for getting names of existing log names (containing dates)
	app.get('/api/server/:uuid/logs', (req, res) => {
		const server = req.body;
		console.log(
			`Getting Log Events for Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		getLogs(req.params.uuid).then((logs) => res.json(logs));
	});

	/* ---------------------------------------- */

	// route for getting a specific log file
	app.get('/api/server/:uuid/log/:log/:file', (req, res) => {
		const server = req.body;
		console.log(
			`Getting Log File ${req.params.log}/${req.params.file} for Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		getLogFile(req.params.uuid, req.params.log, req.params.file).then((
			logFile,
		) => res.json(logFile));
	});

	/* ---------------------------------------- */

	// route for getting the public ip of the host
	app.get('/api/get-public-ip', (req, res) => {
		publicIpv4().then((ipv4) => {
			console.log(`Getting Public IP of this Host: ${ipv4}`);
			res.json({ ipv4 });
		});
	});

	/* ---------------------------------------- */

	// route for getting all servers and their configs
	app.get('/api/get-servers', (req, res) => {
		console.log(`Getting list of Arma Reforger Servers.`);
		getServers().then((servers) => res.json(servers));
	});

	/* ---------------------------------------- */

	// route for getting a specific server
	app.get('/api/server/:uuid', (req, res) => {
		console.log(
			`Getting Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		getServer(req.params.uuid).then((server) => res.json(server));
	});

	/* ---------------------------------------- */

	// route for starting a specific server
	app.get('/api/server/:uuid/start', (req, res) => {
		console.log(
			`Starting Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		// starting ars instance
		const ars = arsList.find((i) => i.uuid === req.params.uuid);
		if (ars) {
			ars.start();
			res.json({ value: true });
		} else {
			throw new Error('Arma Reforger Server not found.');
		}
	});

	/* ---------------------------------------- */

	// route for stopping a specific server
	app.get('/api/server/:uuid/stop', (req, res) => {
		console.log(
			`Stopping Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		// stop running ars instance
		const ars = arsList.find((i) => i.uuid === req.params.uuid);
		if (ars) {
			ars.stop();
			res.json({ value: true });
		} else {
			throw new Error('Arma Reforger Server not found.');
		}
	});

	/* ---------------------------------------- */

	// route for deleting a specific server
	app.get('/api/server/:uuid/delete', (req, res) => {
		console.log(
			`Deleting Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		// delete server and profile
		const profilePath = join(Deno.cwd(), 'profiles', req.params.uuid);
		try {
			Deno.removeSync(profilePath, { recursive: true });
		} catch (error) {
			if (!(error instanceof Deno.errors.NotFound)) {
				throw error;
			}
		}
		const serverPath = join(Deno.cwd(), 'servers', req.params.uuid);
		try {
			Deno.removeSync(serverPath, { recursive: true });
		} catch (error) {
			if (!(error instanceof Deno.errors.NotFound)) {
				throw error;
			}
		}
		const ars = arsList.find((i) => i.uuid === req.params.uuid);
		if (ars) {
			arsList.splice(arsList.indexOf(ars), 1);
			res.json({ value: true });
		} else {
			throw new Error('Arma Reforger Server not found.');
		}
	});

	/* ---------------------------------------- */

	// route for starting the isRunning state of a specific server
	app.get('/api/server/:uuid/isRunning', (req, res) => {
		const ars = arsList.find((i) => i.uuid === req.params.uuid);
		if (!ars) {
			res.json({ value: false });
			console.log(
				`Arma Reforger Server with UUID ${req.params.uuid} is running: ${false}`,
			);
		} else {
			console.log(
				`Arma Reforger Server with UUID ${req.params.uuid} is running: ${ars.isRunning}`,
			);
			res.json({ value: ars.isRunning });
		}
	});

	/* ---------------------------------------- */

	// websocket configuration for server to client push notifications
	app.ws('/', (ws, req: Request) => {
		ws.on('message', (msg: string) => {
			ws.send('pong');
		});

		ws.on('close', () => {
			console.log('WebSocket closed.');
		});

		console.log('WebSocket opened.');
	});

	/* ---------------------------------------- */

	const server = app.listen(3000);

	/* ---------------------------------------- */

	// broadcast all updates related to ars isRunning status to all clients
	const interval = setInterval(() => {
		arsList.forEach((ars) => {
			while (ars.messageQueue.length > 0) {
				const message = ars.messageQueue.splice(0, 1)[0];
				const clients = wsInstance.getWss().clients as WebSocket[];
				clients.forEach((client) => {
					client.send(JSON.stringify(message));
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
		() => {
			console.log('SIGINT!');
			server.close();
			Deno.exit(0);
		},
	);
}
