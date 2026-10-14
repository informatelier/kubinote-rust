import { ref } from 'vue';

export function useSettings(sessionId) {
    const settingsOpen = ref(false);
    const settingsLoading = ref(false);
    const settingsSaved = ref(false);
    const settings = ref({});
    const contextUsage = ref({
        contextWindow: 0, chatContextMaxTokens: 0, occupiedTokens: 0,
        systemTokens: 0, pinnedTokens: 0, historyTokens: 0,
        ragAllowanceTokens: 0, percentOfWindow: 0, percentOfBudget: 0
    });
    const availableModels = ref([]);
    const modelsLoading = ref(false);

    async function loadContextUsage() {
        try {
            const res = await fetch(`/context/usage?sessionId=${encodeURIComponent(sessionId.value || 'default')}`);
            if (res.ok) contextUsage.value = await res.json();
        } catch (e) { /* badge is best-effort */ }
    }

    async function loadModels() {
        modelsLoading.value = true;
        try {
            const res = await fetch('/models');
            if (res.ok) availableModels.value = await res.json();
        } catch (e) {
            /* list is best-effort */
        } finally {
            modelsLoading.value = false;
        }
    }

    async function openSettingsPanel() {
        settingsOpen.value = true;
        settingsSaved.value = false;
        settingsLoading.value = true;
        loadModels();
        try {
            const res = await fetch('/config');
            settings.value = await res.json();
        } catch (e) {
            alert('Config load error: ' + e.message);
        } finally {
            settingsLoading.value = false;
        }
    }

    async function saveSettings() {
        try {
            const res = await fetch('/config', {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(settings.value)
            });
            if (!res.ok) {
                const err = await res.json();
                alert(err.error || 'Save failed');
                return;
            }
            settingsSaved.value = true;
            setTimeout(() => { settingsSaved.value = false; }, 2500);
            loadContextUsage();
        } catch (e) {
            alert('Save error: ' + e.message);
        }
    }

    return {
        settingsOpen, settingsLoading, settingsSaved, settings,
        contextUsage, availableModels, modelsLoading,
        loadContextUsage, loadModels, openSettingsPanel, saveSettings
    };
}
