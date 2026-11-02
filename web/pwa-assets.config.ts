// Icons of the installable app, generated once from public/icon.svg and committed:
//   npx @vite-pwa/assets-generator@2.0.0
// The generator is not a dependency: it brings a native image library that Docker and CI
// would download for nothing. Run it again only when the icon changes. For the same reason
// this file imports nothing: it spells out the generator's "minimal 2023" preset.

// The icon has its own full-bleed red background, so padding is filled with the same red.
const red = '#D62828'

export default {
  headLinkOptions: { preset: '2023' },
  preset: {
    transparent: { sizes: [64, 192, 512], favicons: [[48, 'favicon.ico']], padding: 0 },
    // Android crops "maskable" icons to a circle or a rounded square: a 20% margin keeps the
    // whole train inside.
    maskable: { sizes: [512], padding: 0.2, resizeOptions: { background: red } },
    apple: { sizes: [180], padding: 0, resizeOptions: { background: red } },
  },
  images: ['public/icon.svg'],
}
