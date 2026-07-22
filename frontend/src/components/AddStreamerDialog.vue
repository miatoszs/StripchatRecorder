<!--
    添加主播对话框组件 / Add Streamer Dialog Component

    提供一个模态对话框，允许用户通过输入 Stripchat 用户名来添加新主播到追踪列表。
    提交时调用 streamersStore.addStreamer 并通过 toast 反馈操作结果。

    Provides a modal dialog for users to add a new streamer to the tracking list
    by entering their Stripchat username. Calls streamersStore.addStreamer on submit
    and provides toast feedback for the operation result.

    Emits:
        close - 对话框关闭时触发 / Emitted when dialog is closed
        added - 主播成功添加后触发 / Emitted after streamer is successfully added
-->
<script setup lang="ts">
	import { ref } from "vue";
	import { useStreamersStore } from "../stores/streamers";
	import { useNotify } from "../composables/useNotify";
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogHeader,
		DialogTitle,
		DialogFooter,
	} from "@/components/ui/dialog";
	import { Button } from "@/components/ui/button";
	import { Input } from "@/components/ui/input";
	import { Label } from "@/components/ui/label";
	import { useI18n } from "vue-i18n";

	const emit = defineEmits<{ close: []; added: [] }>();
	const store = useStreamersStore();
	const { toast } = useNotify();
	const { t } = useI18n();
	const username = ref("");
	const loading = ref(false);

	/**
	 * ：、stripchat.com  mirror 。
	 * Extract username from input: supports plain username, stripchat.com URL, or mirror URL.
	 */
	function extractUsername(input: string): string {
		const trimmed = input.trim();
		try {
			const url = new URL(trimmed.startsWith("http") ? trimmed : `https://${trimmed}`);
			// stripchat.com  mirror  /<username>
			const parts = url.pathname.split("/").filter(Boolean);
			if (parts.length > 0) return parts[0];
		} catch {
			// URL，
		}
		return trimmed;
	}

	/**
	 * ：、 addStreamer、。
	 * Submit the form: validate input, call addStreamer, provide feedback.
	 */
	async function submit() {
		const name = extractUsername(username.value);
		if (!name) return;
		loading.value = true;
		try {
			await store.addStreamer(name);
			toast(t("addStreamer.done", { name }), "success");
			emit("added");
		} catch (e) {
			toast(String(e), "error");
		} finally {
			loading.value = false;
		}
	}
</script>

<template>
	<Dialog :open="true" @update:open="(v) => !v && emit('close')">
		<DialogContent class="sm:max-w-95">
			<DialogHeader>
				<DialogTitle>{{ t("addStreamer.title") }}</DialogTitle>
				<DialogDescription class="sr-only">{{ t("addStreamer.description") }}</DialogDescription>
			</DialogHeader>

			<form @submit.prevent="submit" class="flex flex-col gap-4 py-2">
				<div class="flex flex-col gap-2">
					<Label for="username">{{ t("addStreamer.label") }}</Label>
				<Input
						id="username"
						v-model="username"
						:placeholder="t('addStreamer.placeholder')"
						autofocus
						:disabled="loading"
					/>
				</div>

				<DialogFooter>
					<Button type="button" variant="outline" @click="emit('close')">
						{{ t("addStreamer.cancel") }}
					</Button>
					<Button type="submit" :disabled="loading || !username.trim()">
						{{ loading ? t("addStreamer.submitting") : t("addStreamer.submit") }}
					</Button>
				</DialogFooter>
			</form>
		</DialogContent>
	</Dialog>
</template>

