function edgePath(edge) {
    const sx = edge.source.x || 0;
    const sy = edge.source.y || 0;
    const tx = edge.target.x || 0;
    const ty = edge.target.y || 0;
    const dx = tx - sx;
    const dy = ty - sy;
    const length = Math.sqrt(dx * dx + dy * dy) || 1;
    const bend = edge._bend || 0;
    const cx = (sx + tx) / 2 - (dy / length) * bend;
    const cy = (sy + ty) / 2 + (dx / length) * bend;
    return `M${sx},${sy} Q${cx},${cy} ${tx},${ty}`;
}

function edgeLabelPoint(edge) {
    const sx = edge.source.x || 0;
    const sy = edge.source.y || 0;
    const tx = edge.target.x || 0;
    const ty = edge.target.y || 0;
    const dx = tx - sx;
    const dy = ty - sy;
    const length = Math.sqrt(dx * dx + dy * dy) || 1;
    return {
        x: (sx + tx) / 2 - (dy / length) * (edge._bend || 0) / 2,
        y: (sy + ty) / 2 + (dx / length) * (edge._bend || 0) / 2
    };
}

function assignBends(edges) {
    const groups = new Map();
    edges.forEach(edge => {
        const a = String(edge.source);
        const b = String(edge.target);
        const key = a < b ? `${a}|${b}` : `${b}|${a}`;
        const group = groups.get(key) || [];
        group.push(edge);
        groups.set(key, group);
    });
    groups.forEach(group => group.forEach((edge, index) => {
        edge._bend = (index - (group.length - 1) / 2) * 22;
    }));
}

export function renderConceptMap(container, data, options = {}) {
    container.replaceChildren();
    const width = Math.max(container.clientWidth || 0, 640);
    const height = Math.max(container.clientHeight || 0, 480);
    const nodes = (data.nodes || []).map(node => ({ ...node }));
    const edges = (data.edges || []).map(edge => ({ ...edge }));
    assignBends(edges);

    const svg = d3.select(container).append('svg')
        .attr('viewBox', `0 0 ${width} ${height}`)
        .attr('role', 'img')
        .attr('aria-label', `Concept map with ${nodes.length} concepts and ${edges.length} relations`);
    const viewport = svg.append('g');
    const zoom = d3.zoom().scaleExtent([0.15, 5]).on('zoom', event => viewport.attr('transform', event.transform));
    svg.call(zoom);

    const defs = svg.append('defs');
    ['accepted', 'suggested'].forEach(type => {
        defs.append('marker')
            .attr('id', `concept-map-arrow-${type}`)
            .attr('viewBox', '0 -5 10 10')
            .attr('refX', 20)
            .attr('refY', 0)
            .attr('markerWidth', 5)
            .attr('markerHeight', 5)
            .attr('orient', 'auto')
            .append('path')
            .attr('d', 'M0,-5L10,0L0,5')
            .attr('class', `concept-map-arrow ${type}`);
    });

    const link = viewport.append('g').attr('class', 'concept-map-links')
        .selectAll('path').data(edges, edge => edge.id).join('path')
        .attr('class', edge => `concept-map-link ${edge.type}`)
        .attr('marker-end', edge => edge.type === 'affinity' ? null : `url(#concept-map-arrow-${edge.type})`);
    const edgeLabel = viewport.append('g').attr('class', 'concept-map-edge-labels')
        .selectAll('text').data(edges, edge => edge.id).join('text')
        .attr('class', edge => `concept-map-edge-label ${edge.type}`)
        .text(edge => edge.label || edge.type);

    const node = viewport.append('g').attr('class', 'concept-map-nodes')
        .selectAll('g').data(nodes, item => item.id).join('g')
        .attr('class', item => `concept-map-node${item.id === data.focus ? ' focused' : ''}`)
        .attr('tabindex', 0)
        .attr('role', 'button')
        .attr('aria-label', item => `Open concept ${item.name}`)
        .on('click', (_, item) => options.onSelect?.(item))
        .on('keydown', (event, item) => {
            if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                options.onSelect?.(item);
            }
        });
    node.append('circle')
        .attr('r', item => 8 + Math.min(8, Math.sqrt(Math.max(0, item.evidenceCount || 0)) * 2));
    node.append('text').attr('x', 13).attr('y', 4).text(item => item.name);
    node.append('title').text(item => {
        const aliases = item.aliases?.length ? `\nAliases: ${item.aliases.join(', ')}` : '';
        const tags = item.tags?.length ? `\nTags: ${item.tags.map(tag => '#' + tag).join(', ')}` : '';
        return `${item.name}\n${item.evidenceCount || 0} evidence${aliases}${tags}`;
    });

    const simulation = d3.forceSimulation(nodes)
        .force('link', d3.forceLink(edges).id(item => item.id).distance(edge => edge.type === 'affinity' ? 125 : 155).strength(0.45))
        .force('charge', d3.forceManyBody().strength(-420))
        .force('center', d3.forceCenter(width / 2, height / 2))
        .force('collision', d3.forceCollide().radius(item => 28 + Math.min(40, item.name.length * 2)))
        .on('tick', () => {
            link.attr('d', edgePath);
            edgeLabel
                .attr('x', edge => edgeLabelPoint(edge).x)
                .attr('y', edge => edgeLabelPoint(edge).y);
            node.attr('transform', item => `translate(${item.x || 0},${item.y || 0})`);
        });

    node.call(d3.drag()
        .on('start', (event, item) => {
            if (!event.active) simulation.alphaTarget(0.25).restart();
            item.fx = item.x;
            item.fy = item.y;
        })
        .on('drag', (event, item) => {
            item.fx = event.x;
            item.fy = event.y;
        })
        .on('end', (event, item) => {
            if (!event.active) simulation.alphaTarget(0);
            item.fx = null;
            item.fy = null;
        }));

    function fit(duration = 300) {
        if (!nodes.length) return;
        const bounds = viewport.node().getBBox();
        if (!bounds.width || !bounds.height) return;
        const scale = Math.min(2, 0.88 / Math.max(bounds.width / width, bounds.height / height));
        const x = width / 2 - scale * (bounds.x + bounds.width / 2);
        const y = height / 2 - scale * (bounds.y + bounds.height / 2);
        svg.transition().duration(duration).call(zoom.transform, d3.zoomIdentity.translate(x, y).scale(scale));
    }

    function focusNode(id) {
        const item = nodes.find(candidate => candidate.id === id);
        if (!item || !Number.isFinite(item.x) || !Number.isFinite(item.y)) return;
        node.classed('focused', candidate => candidate.id === id);
        svg.transition().duration(250).call(
            zoom.transform,
            d3.zoomIdentity.translate(width / 2 - item.x * 1.35, height / 2 - item.y * 1.35).scale(1.35)
        );
    }

    const fitTimer = window.setTimeout(() => fit(450), 700);
    return {
        fit,
        focusNode,
        destroy() {
            window.clearTimeout(fitTimer);
            simulation.stop();
            container.replaceChildren();
        }
    };
}
