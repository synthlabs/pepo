const config = {
	content: ['./src/**/*.{html,js,svelte,ts}'],

	theme: {
		extend: {}
	},

	plugins: [require("daisyui")],

	daisyui: {
		styled: true,
		themes: [
			"light",
			"dark",
			{
				glitchv1: { // https://coolors.co/a970ff-0d1821-344966-28afb0-ddcecd
					primary: "#A970FF",
					secondary: "#28AFB0",
					accent: "#DDCECD",
					neutral: "#344966",
					"base-100": "#0d1821",
					info: "#3ABFF8",
					success: "#36D399",
					warning: "#FBBD23",
					error: "#F87272",
				},
				glitchv2: { // https://coolors.co/a970ff-e2b1b1-a2faa3-2a303c-191d24
					"primary": "#A970FF",
					"secondary": "#E2B1B1",
					"accent": "#A2FAA3",
					"neutral": "#191D24",
					"base-100": "#2A303C",
					"info": "#3ABFF8",
					"success": "#36D399",
					"warning": "#FBBD23",
					"error": "#F87272",
				},
			},
		],
		base: true,
		utils: true,
		logs: true,
		rtl: false,
		prefix: "",
		darkTheme: "glitchv2",
	},
};

module.exports = config;

