// SSR Entry Point - demonstrates code splitting with dynamic imports

export default async function render(props) {
  console.log("Starting SSR render with props:", props);

  // These will be code-split into separate chunks by esbuild
  const { Header } = await import("./components/header.js");
  const { Footer } = await import("./components/footer.js");

  // Dynamic page loading based on props
  let pageContent;
  const page = props.page || "home";

  try {
    // This creates a separate chunk for each page
    const pageModule = await import(`./pages/${page}.js`);
    pageContent = await pageModule.render(props);
  } catch (e) {
    console.error("Failed to load page:", page, e.message);
    pageContent = "<main><h1>404 - Page Not Found</h1></main>";
  }

  const html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${props.title || "SSR App"}</title>
</head>
<body>
  ${Header({ title: props.title })}
  ${pageContent}
  ${Footer()}
</body>
</html>`;

  console.log("SSR render complete");
  return html;
}
