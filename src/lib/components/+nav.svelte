<script lang="ts">
	import { user } from '$lib/store/user';
	import Tabs from '$lib/components/+tabs.svelte';
</script>

<div class="flex flex-row w-full items-center bg-base-300">
	{#if $user}
		<Tabs />
	{/if}

	<div class="flex gap-2 pr-1 justify-end items-end">
		<div class="dropdown dropdown-end">
			<label tabindex="0" class="btn btn-ghost btn-circle avatar">
				<div class="w-9 rounded-full">
					{#if $user}
						<img src={$user?.profilePictureUrl} alt={$user?.displayName} />
					{:else}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							fill="none"
							viewBox="0 0 24 24"
							stroke-width="1.5"
							stroke="currentColor"
						>
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								d="M17.982 18.725A7.488 7.488 0 0012 15.75a7.488 7.488 0 00-5.982 2.975m11.963 0a9 9 0 10-11.963 0m11.963 0A8.966 8.966 0 0112 21a8.966 8.966 0 01-5.982-2.275M15 9.75a3 3 0 11-6 0 3 3 0 016 0z"
							/>
						</svg>
					{/if}
				</div>
			</label>
			<ul
				tabindex="0"
				class="menu menu-compact dropdown-content mt-3 p-2 shadow bg-base-300 rounded-box w-52 z-50"
			>
				<li><a href="/settings">Settings</a></li>
			</ul>
		</div>
	</div>
</div>

<style>
</style>
