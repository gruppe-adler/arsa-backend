import '@std/dotenv/load'; // Automatically load environment variables from a `.env` file

import { join } from '@std/path';

import { ArmaReforgerServer } from "./ars.ts";

import { getServers } from './utils.ts';

// @deno-types="npm:@types/express@4.17.15"
import express from "npm:express@4.18.2";
import cors from "npm:cors@2.8.5"

//TODO(@y0014984): oak verwenden? https://jsr.io/@oak/oak

if (import.meta.main) {
	const running: ArmaReforgerServer[] = []; // running servers

	const app = express();

	app.use(cors())

	app.use(express.json()) // parsing JSON in req; result available in req.json

	app.post("/api/add-server/", (req, res) => {
		const server = req.body;
		const uuid = crypto.randomUUID();
		server.uuid = uuid;
		console.log(`Adding Arma Reforger Server with UUID: ${uuid}`);
		console.log(req.body);
		Deno.mkdir(`servers/${uuid}`)
			.then(() => {
				Deno.writeTextFile(`./servers/${uuid}/server.json`, JSON.stringify(server, null, 2))
			})
			.then(() => {
				Deno.writeTextFile(`./servers/${uuid}/config.json`, JSON.stringify(server.config, null, 2))
			})
			.then(() => res.json({uuid}));
	});

	app.get("/api/get-servers", (req, res) => {
		console.log(`Getting list of Arma Reforger Servers.`);
		getServers('./servers').then(servers => res.json(servers));
	});

	app.get("/api/server/:uuid/start", (req, res) => {
		console.log(`Starting Arma Reforger Server with UUID: ${req.params.uuid}`);
		// create ars instance and start it
		const ars = new ArmaReforgerServer(req.params.uuid);
		running.push(ars);
		res.json({result: true});
	});

	app.get("/api/server/:uuid/stop", (req, res) => {
		console.log(`Stopping Arma Reforger Server with UUID: ${req.params.uuid}`);
		// create ars instance and start it
		const ars = running.find(i => i.uuid === req.params.uuid);
		ars?.stop();
		res.json({result: true});
	});
	
	const server = app.listen(8000);

	// things to do if <CTRL+C> is pressed
	Deno.addSignalListener(
		'SIGINT',
		() => {
			console.log('SIGINT!');
			server.close();
		},
	);
}
