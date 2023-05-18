<script lang="ts">
	import { user } from '$lib/store/user';
</script>

<div class="flex w-full items-center h-15 bg-base-200 shadow">
	<div class="flex-1 pl-1">
		<a href="/" class="btn btn-ghost btn-circle">
			<div class="w-7">
				<img alt="Pepo logo" src="/pepo-flat.png" />
			</div>
		</a>
	</div>
	{#if $user}
		<div class="flex-none gap-2">
			<span class="btn btn-ghost btn-circle avatar">
				<div class="w-8 rounded-full">
					<img src={$user?.profilePictureUrl} alt={$user?.displayName} />
				</div>
			</span>
		</div>
	{/if}
</div>
