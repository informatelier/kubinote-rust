/**
 * Executes a notebook mutation against the latest revision exposed by the server.
 * The backend still performs the atomic revision check, so a concurrent mutation
 * between these two requests is reported as a normal 409 conflict.
 */
export async function governedFetch(input, init = {}, revisionInput = '/governance') {
    const status = await fetch(revisionInput, {
        cache: 'no-store',
        headers: { 'Accept': 'application/json' }
    });
    if (!status.ok) throw new Error(`Cannot read notebook revision (HTTP ${status.status})`);

    let revision = status.headers.get('ETag');
    if (!revision) {
        const body = await status.json();
        if (!Number.isInteger(body.revision) || body.revision < 0) {
            throw new Error('Notebook revision is missing from the governance response');
        }
        revision = `"${body.revision}"`;
    }

    const headers = new Headers(init.headers || {});
    headers.set('If-Match', revision);
    return fetch(input, { ...init, headers });
}
