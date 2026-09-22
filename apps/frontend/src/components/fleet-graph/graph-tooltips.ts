import type { GraphNode } from './build-force-graph-data';

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function formatTimestamp(value?: string | null): string | undefined {
  if (!value) return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

export function formatExternalTooltip(node: GraphNode): string {
  if (node.details && node.details.length > 0) {
    return formatNodeTooltip(node.name, node.details);
  }

  const connection = node.connection;
  if (!connection) return '';

  const rows = [
    ['Type', connection.device_type ?? node.visualName],
    ['Connection', connection.connection_type],
    ['Status', connection.status],
    ['Address', connection.address],
    ['Last seen', formatTimestamp(connection.last_seen_at)],
    ['First seen', formatTimestamp(connection.first_seen_at)],
    ['External ID', connection.external_id],
    ['Source', connection.source],
    ['Device ID', connection.device_id],
  ].filter((row): row is [string, string] => Boolean(row[1]));

  if (rows.length === 0) return escapeHtml(node.name);

  return formatNodeTooltip(
    node.name,
    rows.map(([label, value]) => ({ label, value })),
  );
}

export function formatNodeTooltip(
  name: string,
  rows: Array<{ label: string; value: string }>,
): string {
  return `
    <div class="fleet-graph-node-tooltip">
      <div class="fleet-graph-node-tooltip-title">${escapeHtml(name)}</div>
      ${rows
        .map(
          ({ label, value }) => `
            <div class="fleet-graph-node-tooltip-row">
              <span>${escapeHtml(label)}</span>
              <strong>${escapeHtml(value)}</strong>
            </div>
          `,
        )
        .join('')}
    </div>
  `;
}
