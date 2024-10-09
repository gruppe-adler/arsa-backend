import '@std/dotenv/load'; // Automatically load environment variables from a `.env` file
import { join } from '@std/path';

// @deno-types="npm:@types/express@4.17.15"
import express from 'npm:express@^4.17.15';
// @deno-types="npm:@types/express-ws@^3.0.5"
import expressWs from 'npm:express-ws@^5.0.2';
import cors from 'npm:cors@2.8.5';
import { publicIpv4 } from 'npm:public-ip@7.0.1';

import { ArmaReforgerServer } from './ars.ts';
import { getServer, getServers, getLogs, getLogFile } from './utils.ts';

if (import.meta.main) {
/* 	try {
		Deno.mkdirSync('profiles')
		Deno.mkdirSync('servers')
	} catch (error) {
		console.log(`Can't create folders. Error: ${error}`)
	} */
	
	const started: ArmaReforgerServer[] = []; // started servers

	const app = express();

	const ws = expressWs(app);

	app.use(cors());

	app.use(express.json()); // parsing JSON in req; result available in req.json

	app.post('/api/add-server/', (req, res) => {
		const server = req.body;
		const uuid = crypto.randomUUID();
		server.uuid = uuid;
		console.log(`Adding Arma Reforger Server with UUID: ${uuid}`);
		console.log(req.body);
		Deno.mkdir(`servers/${uuid}`)
			.then(() => {
				Deno.writeTextFile(
					`./servers/${uuid}/server.json`,
					JSON.stringify(server, null, 2),
				);
			})
			.then(() => {
				Deno.writeTextFile(
					`./servers/${uuid}/config.json`,
					JSON.stringify(server.config, null, 2),
				);
			})
			.then(() => res.json({ uuid }));
	});

	app.post('/api/server/:uuid/update', (req, res) => {
		const server = req.body;
		console.log(
			`Updating Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		console.log(req.body);
		Deno.writeTextFile(
			`./servers/${req.params.uuid}/server.json`,
			JSON.stringify(server, null, 2),
		);
		Deno.writeTextFile(
			`./servers/${req.params.uuid}/config.json`,
			JSON.stringify(server.config, null, 2),
		);
		res.json({ value: true });
	});

	app.get('/api/server/:uuid/logs', (req, res) => {
		const server = req.body;
		console.log(
			`Getting Log Events for Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		getLogs(req.params.uuid).then((logs) => res.json(logs));
	});

	app.get('/api/server/:uuid/log/:log/:file', (req, res) => {
		const server = req.body;
		console.log(
			`Getting Log File ${req.params.log}/${req.params.file} for Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		getLogFile(req.params.uuid, req.params.log, req.params.file).then((logFile) => res.json(logFile));
	});

	app.get('/api/get-public-ip', (req, res) => {
		publicIpv4().then((ipv4) => {
			console.log(`Getting Public IP of this Host: ${ipv4}`);
			res.json({ ipv4 });
		});
	});

	app.get('/api/get-servers', (req, res) => {
		console.log(`Getting list of Arma Reforger Servers.`);
		getServers().then((servers) => res.json(servers));
	});

	app.get('/api/server/:uuid', (req, res) => {
		console.log(
			`Getting Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		getServer(req.params.uuid).then((server) =>
			res.json(server)
		);
	});

	app.get('/api/server/:uuid/start', (req, res) => {
		console.log(
			`Starting Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		// create ars instance and start it
		const ars = new ArmaReforgerServer(req.params.uuid);
		started.push(ars);
		res.json({ value: true });
	});

	app.get('/api/server/:uuid/stop', (req, res) => {
		console.log(
			`Stopping Arma Reforger Server with UUID: ${req.params.uuid}`,
		);
		// stop running ars instance
		const ars = started.find((i) => i.uuid === req.params.uuid);
		if(ars){
			ars.stop();
			started.splice(started.indexOf(ars), 1);
			res.json({ value: true });
		} else {
			res.json({ value: false });
		}
	});

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
		res.json({ value: true });
	});

	app.get('/api/server/:uuid/isRunning', (req, res) => {
		// create ars instance and start it
		const ars = started.find((i) => i.uuid === req.params.uuid);
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

	app.ws('/', (ws, req: Request) => {
		ws.on('message', (msg: string) => {
		  console.log(`WebSocket Message: ${msg}`);
		  ws.send('pong');
		});

		ws.on('close', () => {
			clearInterval(interval);
			console.log('WebSocket closed.');
		});
		
		const interval = setInterval(() => {
			started.forEach(ars => {
				while(ars.messageQueue.length > 0) {
					const message = ars.messageQueue.splice(0, 1);
					ws.send(JSON.stringify(message[0]));
					console.log(JSON.stringify(message[0]));
				}
			});
		}, 3_000);

		console.log('WebSocket opened.');
	});

	const server = app.listen(3000);

	// things to do if <CTRL+C> is pressed
	Deno.addSignalListener(
		'SIGINT',
		() => {
			console.log('SIGINT!');
			server.close();
			//webSocketServer.shutdown().then(() => console.log('WebSocketServer shutdown done'));
			Deno.exit(0);
		},
	);
}
