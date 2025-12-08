// Home page - separate chunk

export function render(props) {
  return `<main>
    <h2>Welcome Home</h2>
    <p>This page was server-side rendered in a secure sandbox.</p>
    <p>User: ${props.user || "Guest"}</p>
    <p>No filesystem, network, or environment access is possible from this code.</p>
  </main>`;
}
