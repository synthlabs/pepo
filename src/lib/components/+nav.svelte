<script lang="ts">
	import { user } from '$lib/store/user';
	import InputNav from '$lib/components/+inputnav.svelte';
</script>

<div class="my-nav flex-shrink-0 h-14 bg-base-300 shadow">
	<div class="pl-1 w-1/2 justify-start">
		<a href="/" class="btn btn-ghost btn-circle">
			<div class="w-7">
				<img alt="Pepo logo" src="/pepo-flat.png" />
			</div>
		</a>
	</div>

	{#if $user}
		<div class="flex-shrink-0 w-1/2"><InputNav /></div>

		<div class="gap-2 pr-1 w-1/2 justify-end items-end">
			<div class="dropdown dropdown-end">
				<!-- svelte-ignore a11y-no-noninteractive-tabindex -->
				<label tabindex="0" class="btn btn-ghost btn-circle avatar">
					<div class="w-9 rounded-full">
						<img src={$user?.profilePictureUrl} alt={$user?.displayName} />
					</div>
				</label>
				<ul
					tabindex="0"
					class="menu menu-compact dropdown-content mt-3 p-2 shadow bg-base-300 rounded-box w-52"
				>
					<li><a href="/settings">Settings</a></li>
				</ul>
			</div>
		</div>
	{/if}
</div>

<style>
	.my-nav {
		display: flex;
		align-items: center;
		height: 3.5rem; /* 56px */
		width: 100%;
	}
	:where(.my-nav > *) {
		display: inline-flex;
		align-items: center;
	}
</style>
