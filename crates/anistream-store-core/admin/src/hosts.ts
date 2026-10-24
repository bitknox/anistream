// How to update the store, for each place it can run. Text in backticks is set as code.

export const UPDATING = 'https://anistream.tv/docs/plugins/updating-a-store/';

export interface HostSteps {
	/** The host's name, as a sentence would use it. */
	name: string;
	/** What updates it, a step each. */
	steps: string[];
	/** Said first when the release starts a new major version. */
	major?: string;
}

const IMAGE_MAJOR =
	'This release starts a new major version, which the image tag `:2` does not follow. Read the release notes, then change the tag to the new major where it is written down.';

const WORKFLOW = [
	"In your copy of the template on GitHub, open Actions and run Update the store. It copies this release's template over yours, keeps your settings file, and pushes, which redeploys the store.",
	'The first time, move `update-store.yml` from the top of your repository to `.github/workflows/` and commit it, so the Actions tab lists it.',
];

const HOSTS: Record<string, HostSteps> = {
	docker: {
		name: 'Docker',
		steps: [
			'In the folder with `compose.yaml`, run `docker compose pull`, then `docker compose up -d`.',
			'The store restarts on the new image and brings its database up to date as it starts.',
		],
		major: `${IMAGE_MAJOR} For Docker, that is the \`image:\` line in \`compose.yaml\`.`,
	},
	fly: {
		name: 'Fly.io',
		steps: ['In the directory you launched from, run `fly deploy`. It pulls the current image.'],
		major: `${IMAGE_MAJOR} For Fly.io, that is \`image\` in \`fly.toml\`.`,
	},
	render: {
		name: 'Render',
		steps: [
			'Render does not redeploy on its own. Open the service, choose Manual Deploy, then Clear build cache & deploy.',
			'The store stops for a few seconds while the disk moves to the new instance.',
		],
		major: `${IMAGE_MAJOR} For Render, that is the \`FROM\` line in \`deploy/render/Dockerfile\` of the repository the service builds from.`,
	},
	railway: {
		name: 'Railway',
		steps: [
			'With Auto Updates on, Railway redeploys in the maintenance window you picked. To update now, open the service and choose Redeploy.',
			'To turn Auto Updates on: Settings, Source, Configure Auto Updates.',
		],
		major: `${IMAGE_MAJOR} For Railway, that is the image under the service's Settings, Source.`,
	},
	heroku: {
		name: 'Heroku',
		steps: [
			'From a clone of the repository, run `heroku git:remote -a <app>`, then `git pull` and `git push heroku main`. The build starts from the current image.',
		],
		major: `${IMAGE_MAJOR} For Heroku, that is the \`FROM\` line in \`deploy/heroku/Dockerfile\`.`,
	},
	cloudflare: { name: 'Cloudflare', steps: WORKFLOW },
	vercel: { name: 'Vercel', steps: WORKFLOW },
	netlify: { name: 'Netlify', steps: WORKFLOW },
	anistream: {
		name: 'anistream',
		steps: [
			'This store runs inside anistream, with `anistream store serve`. Update anistream and start the store again.',
		],
	},
	binary: {
		name: 'this machine',
		steps: [
			"Replace `anistream-store` with the release's, then start it again with the same directory.",
		],
	},
};

const UNKNOWN: HostSteps = {
	name: 'its host',
	steps: [
		'Deploy the new release the way this store was deployed. The guide has the steps for each host.',
	],
};

export function hostSteps(host?: string): HostSteps {
	return (host && HOSTS[host]) || UNKNOWN;
}
