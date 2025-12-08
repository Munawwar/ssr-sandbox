// Header component - will be split into its own chunk

export function Header(props) {
  return `<header>
    <nav>
      <h1>${props.title || "My App"}</h1>
      <ul>
        <li><a href="/">Home</a></li>
        <li><a href="/about">About</a></li>
        <li><a href="/contact">Contact</a></li>
      </ul>
    </nav>
  </header>`;
}
