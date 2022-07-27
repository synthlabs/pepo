<script>
	let messages = [
		{ ts: '7:21:48 pm', username: 'CourtofJester', message: 'KEKW ' },
		{ ts: '7:21:48 pm', username: 'cromagnetron', message: 'ESCORT MISSION catJAM ' },
		{ ts: '7:21:48 pm', username: 'Mantis_MD', message: 'COPIUM ' },
		{ ts: '7:21:48 pm', username: 'ThePorkupine', message: 'Sadge ' },
		{ ts: '7:21:48 pm', username: 'kaofishy', message: 'hasPOGGIES RARE DROP IN GAME ' },
		{ ts: '7:21:49 pm', username: 'gizmomacks', message: 'peepoPopcorn wdym ' },
		{ ts: '7:21:49 pm', username: 'IsaSolaris', message: 'KEKW Its the best part ' },
		{ ts: '7:21:49 pm', username: 'ammmar34', message: 'that is the fun ' },
		{ ts: '7:21:49 pm', username: 'viol3tnebula', message: 'hasL I like watchin you game ' },
		{ ts: '7:21:49 pm', username: 'J1bJab', message: 'jk hasL ' },
		{ ts: '7:21:50 pm', username: 'topherr_da_gopher', message: 'git gud ' },
		{ ts: '7:21:50 pm', username: 'Roxforr', message: 'I Never NOPERS ' },
		{ ts: '7:21:50 pm', username: 'sifthegreatgreywolfx', message: 'comes with the job :p ' },
		{ ts: '7:21:50 pm', username: 'theHubajoob', message: 'perhaps you should be better? ' },
		{ ts: '7:21:51 pm', username: 'uumut01', message: 'Sadge ' },
		{ ts: '7:21:51 pm', username: 'SchrodingersCathulhu', message: 'be better has Sadge ' },
		{ ts: '7:21:51 pm', username: 'geopan94', message: 'widepeepoSad ' },
		{ ts: '7:21:51 pm', username: 'Schmaltzy32', message: 'my gamer hasL ' },
		{ ts: '7:21:51 pm', username: 'CammyD', message: 'thats the point ' },
		{ ts: '7:21:51 pm', username: 'timothy_champagne', message: 'ur usually pretty good at them ' },
		{
			ts: '7:21:51 pm',
			username: 'Cmac0801',
			message: "yes that's why we want you to play games "
		},
		{ ts: '7:21:51 pm', username: 'justtrosie', message: 'with love ' },
		{ ts: '7:21:52 pm', username: 'PrettyKrazy', message: 'thats the point ' },
		{
			ts: '7:21:52 pm',
			username: 'condurrr',
			message: "there's a serious lack of meowing @HasanAbi Madge "
		},
		{ ts: '7:21:52 pm', username: 'darksushi810', message: 'Okayge ' },
		{ ts: '7:21:52 pm', username: 'joepicker', message: 'bruh you terrible ' },
		{ ts: '7:21:52 pm', username: 'Lobaritmo', message: 'chat pls be nice Sadge ' },
		{ ts: '7:21:52 pm', username: 'elhijodejackslid', message: 'NOOO NO MORE GAMING ' },
		{ ts: '7:21:52 pm', username: 'satorinin', message: "PogO That's part of the fun" }
	];
</script>

<div>
	{#each messages as msg}
		<div>
			<span class="text-primary text-opacity-80">{msg.ts}</span>
			<span class="text-secondary">{msg.username}</span><span>:</span>
			<span>{msg.message}</span>
		</div>
	{/each}
</div>
