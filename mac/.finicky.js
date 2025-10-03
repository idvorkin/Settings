// ~/.finicky.js

export default {
  defaultBrowser: "Microsoft Edge",

  // Optional: Disable logging requests to disk
  logRequests: false,

  // Rewrite all http URLs to https
  rewrite: [
    {
      match: ({ url }) => url.protocol === "http:",
      url: ({ url }) => url.href.replace(/^http:/, "https:"),
    },
  ],

  handlers: [
    {
      // Open apple.com and example.org URLs in Safari
      match: [
        /^https?:\/\/(www\.)?apple\.com\/.*/i,
        /^https?:\/\/(www\.)?example\.org\/.*/i,
      ],
      browser: "Safari",
    },
    {
      // Gmail and Google Mail in Edge
      match: [
        /gmail\.com/i,
        /mail\.google\.com/i,
      ],
      browser: "Microsoft Edge",
    },
    {
      // Figma and workplace.com in Chrome
      match: [
        /figma/i,
        /workplace\.com/i,
      ],
      browser: "Google Chrome",
    },
    {
      // thefacebook.com in Chrome
      match: [
        /thefacebook\.com/i,
      ],
      browser: "Google Chrome",
    },
    {
      // Facebook SSO and Zoom in Chrome
      match: [
        /facebook\.csod\.com/i,
        /facebook\.zoom\.us/i,
        /meta\.zoom\.us/i,
      ],
      browser: "Google Chrome",
    },
    {
      // Meta/FB internal tools in Chrome
      match: [
        /fburl\.com/i,
        /internalfb\.com/i,
        /fb\.facebook\.com/i,
        /fb\.workplace\.com/i,
        /docs\.google\.com/i,
        /figma/i,
        /fb\.okta\.com/i,
        /drive\.google\.com\/drive\/folders/i,
        /fb\.quip\.com/i,
      ],
      browser: "Google Chrome",
    },
  ],
};
