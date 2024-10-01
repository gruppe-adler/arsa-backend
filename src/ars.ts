import { join } from '@std/path';

import { ProcessManager } from './process-manager.ts';

export class ArmaReforgerServer {
    uuid: string;
    installPath: string;
    process: ProcessManager;

    constructor(uuid: string) {
        if(!uuid) { throw new Error('UUID is missing.'); };

        this.uuid = uuid;

        // get installation directory from environment variable containing ArmaReforgerServer executable
        this.installPath = Deno.env.get('ARMA_REFORGER_SERVER_INSTALL_DIR') ||
		    '~/.local/share/Steam/steamapps/common/Arma\ Reforger\ Server';

        // prepare arguments
        const args = [
            `-i ${this.uuid}`, 
            `-d "${this.installPath}"`, 
            `-c "${join(Deno.cwd(), 'servers', this.uuid, 'config.json')}"`, 
            `-p "${join(Deno.cwd(), 'profiles', this.uuid)}"`]

        this.process = new ProcessManager(Deno.cwd(), 'ars-start.sh', args);

        console.log('Arma Reforger Server starting initiated.');
    }

    stop() {
        // get the pid of the Arma Reforger Process from the pid file created by the shell script ars-start.sh
        // then kill that process
        Deno.readTextFile(join(this.installPath, `${this.uuid}.pid`))
            .then(pid => {
                Deno.kill(parseInt(pid));
                console.log('Arma Refoger Server stopped.');
            });
    }

    async isRunning() : Promise<boolean> {
        const pid = await Deno.readTextFile(join(this.installPath, `${this.uuid}.pid`));
        const command = new Deno.Command('pgrep', { cwd: this.installPath, args: ['-F', this.uuid + '.pid']});
        const result = await command.output();
        return new TextDecoder().decode(result.stdout) !== '' ? true : false;
    }
}