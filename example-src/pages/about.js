// About page - separate chunk

export function render(props) {
  return `<main>
    <h2>About Us</h2>
    <p>This is a demo of sandboxed SSR execution using deno_core.</p>
    <h3>Security Features:</h3>
    <ul>
      <li>No filesystem access</li>
      <li>No network access</li>
      <li>No environment variables</li>
      <li>No child process spawning</li>
      <li>Dynamic imports restricted to sandbox directory</li>
    </ul>
  </main>`;
}
