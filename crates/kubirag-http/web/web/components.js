import { inject } from 'vue';

async function loadTemplate(name) {
    const response = await fetch(`/web/components/${name}.html`);
    if (!response.ok) {
        throw new Error(`Unable to load component template "${name}": HTTP ${response.status}`);
    }
    return response.text();
}

function statefulComponent(template) {
    return {
        setup() {
            return inject('appState');
        },
        template
    };
}

const [
    headerTemplate,
    sourcesTemplate,
    workspaceTemplate,
    studioTemplate,
    vueModalsTemplate
] = await Promise.all([
    loadTemplate('header'),
    loadTemplate('sources'),
    loadTemplate('workspace'),
    loadTemplate('studio'),
    loadTemplate('vue-modals')
]);

export const AppHeader = statefulComponent(headerTemplate);
export const SourcesPanel = statefulComponent(sourcesTemplate);
export const WorkspacePanel = statefulComponent(workspaceTemplate);
export const StudioPanel = statefulComponent(studioTemplate);
export const VueModals = statefulComponent(vueModalsTemplate);
