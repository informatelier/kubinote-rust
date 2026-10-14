import { ref } from 'vue';
import { governedFetch } from '/web/modules/governance.js';

export function useUploads({ refreshSources, loadTermIndex }) {
    const uploading = ref(false);
    const uploadError = ref('');
    const uploadSuccess = ref('');
    const draggingOver = ref(false);
    const fileInputRef = ref(null);
    const pdfModal = ref({
        open: false, savedPath: '', filename: '', pagesPerChunk: 20,
        groupName: '', existingChunks: 0, reusedExisting: false, reimportMode: 'auto'
    });
    const addSourceModal = ref({ open: false, url: '', scraping: false });

    function clearMessage(message, delay) {
        setTimeout(() => { message.value = ''; }, delay);
    }

    async function refreshAfterImport() {
        await refreshSources();
        await loadTermIndex();
    }

    async function confirmPdfChunked() {
        pdfModal.value.open = false;
        uploading.value = true;
        uploadError.value = '';
        try {
            const formData = new FormData();
            formData.append('filePath', pdfModal.value.savedPath);
            formData.append('pagesPerChunk', pdfModal.value.pagesPerChunk);
            formData.append('groupName', pdfModal.value.groupName);
            formData.append('reimportMode', pdfModal.value.reimportMode);
            const res = await governedFetch('/documents/pdf-chunked', { method: 'POST', body: formData });
            const data = await res.json();
            if (!res.ok) {
                uploadError.value = data.error || 'Chunking failed';
                clearMessage(uploadError, 4000);
                return;
            }
            uploadSuccess.value = `${data.chunksCreated} chunks added${data.chunksRemoved > 0 ? ` (${data.chunksRemoved} removed)` : ''}`;
            await refreshAfterImport();
            clearMessage(uploadSuccess, 3000);
        } catch (e) {
            uploadError.value = 'Error: ' + e.message;
            clearMessage(uploadError, 4000);
        } finally {
            uploading.value = false;
            if (fileInputRef.value) fileInputRef.value.value = '';
        }
    }

    async function scrapeDocument() {
        const url = addSourceModal.value.url.trim();
        if (!url) return;
        addSourceModal.value.scraping = true;
        uploadError.value = '';
        uploadSuccess.value = '';
        try {
            const formData = new FormData();
            formData.append('url', url);
            const res = await governedFetch('/documents/scrape', { method: 'POST', body: formData });
            const data = await res.json();
            if (!res.ok) {
                uploadError.value = data.error || 'Scrape failed';
                clearMessage(uploadError, 4000);
                return;
            }
            uploadSuccess.value = `"${data.title}" added (${data.charCount} chars)`;
            addSourceModal.value.open = false;
            addSourceModal.value.url = '';
            await refreshAfterImport();
            clearMessage(uploadSuccess, 3000);
        } catch (e) {
            uploadError.value = 'Error: ' + e.message;
            clearMessage(uploadError, 4000);
        } finally {
            addSourceModal.value.scraping = false;
        }
    }

    async function uploadDocument(file, targetDir = '') {
        const ext = (file.name.split('.').pop() || '').toLowerCase();
        if (!['md', 'txt', 'pdf'].includes(ext)) {
            uploadError.value = `Unsupported format: .${ext}`;
            clearMessage(uploadError, 3500);
            return;
        }
        uploading.value = true;
        uploadError.value = '';
        uploadSuccess.value = '';
        try {
            const formData = new FormData();
            formData.append('file', file);
            if (targetDir) formData.append('targetDir', targetDir);
            const res = await governedFetch('/documents/upload', { method: 'POST', body: formData });
            const data = await res.json();
            if (res.status === 202 && data.requiresConfig) {
                pdfModal.value = {
                    open: true,
                    savedPath: data.savedPath,
                    filename: file.name,
                    pagesPerChunk: data.defaultPagesPerChunk || 20,
                    groupName: data.suggestedGroupName || file.name.replace(/\.pdf$/i, ''),
                    existingChunks: data.existingChunks || 0,
                    reusedExisting: data.reusedExisting === true,
                    reimportMode: 'auto'
                };
                uploading.value = false;
                return;
            }
            if (!res.ok) {
                uploadError.value = data.error || 'Upload failed';
                clearMessage(uploadError, 4000);
                return;
            }
            const reusedExisting = res.headers.get('X-QB-Source-Reused') === 'true';
            const reindexed = res.headers.get('X-QB-Source-Reindexed') === 'true';
            uploadSuccess.value = reindexed
                ? `"${file.name}" reindexed`
                : reusedExisting
                ? `"${file.name}" registered from the existing file`
                : `"${data.filename || file.name}" added`;
            await refreshAfterImport();
            clearMessage(uploadSuccess, 3000);
        } catch (e) {
            uploadError.value = 'Error: ' + e.message;
            clearMessage(uploadError, 4000);
        } finally {
            uploading.value = false;
            if (fileInputRef.value) fileInputRef.value.value = '';
        }
    }

    function handleFileSelect(event) {
        const file = event.target.files[0];
        if (file) uploadDocument(file);
    }

    function handleDrop(event) {
        event.preventDefault();
        draggingOver.value = false;
        const file = event.dataTransfer.files[0];
        if (file) uploadDocument(file);
    }

    function handleDragOver(event) {
        event.preventDefault();
        draggingOver.value = true;
    }

    function handleDragLeave(event) {
        if (!event.currentTarget.contains(event.relatedTarget)) draggingOver.value = false;
    }

    return {
        uploading, uploadError, uploadSuccess, draggingOver, fileInputRef,
        pdfModal, addSourceModal, confirmPdfChunked, scrapeDocument, uploadDocument,
        handleFileSelect, handleDrop, handleDragOver, handleDragLeave
    };
}
