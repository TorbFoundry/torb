const withMarkdoc = require('@markdoc/next.js');
module.exports =
  withMarkdoc({mode: "static", schemaPath: "./src/markdoc"}/* config: https://markdoc.io/docs/nextjs#options */)({
    pageExtensions: ['js', 'jsx', 'ts', 'tsx', 'md', 'mdoc'],
    output: "standalone",
    reactStrictMode: true
  });
