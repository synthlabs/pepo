<script lang="ts">
	import { user } from '$lib/store/user';
</script>

<div class="flex w-full items-center h-15 bg-base-300 shadow">
	<div class="flex-1 pl-1">
		<a href="/" class="btn btn-ghost btn-circle">
			<div class="w-7">
				<img alt="Pepo logo" src="/pepo-flat.png" />
			</div>
		</a>
	</div>

	{#if $user}
		<div class="flex-none gap-2 pr-1">
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
