// ~/.finicky.js

module.exports = {
  defaultBrowser: "Microsoft Edge",
  //defaultBrowser: "Google Chrome",
  rewrite: [
    {
      // Redirect all urls to use https
      match: ({ url }) => url.protocol === "http",
      url: { protocol: "http" },
    },
  ],
  handlers: [
    {
      // Open apple.com and example.org urls in Safari
      match: ["apple.com/*", "example.org/*"],
      browser: "Safari",
    },
    {
      match: ["*gmail.com/*", "*mail.google.com*"],
      browser: "Microsoft Edge",
    },
    {
      // Open any url that includes the string "workplace" in Firefox
      match: ["*.workplace.com/*", "*figma*"],
      browser: "Google Chrome",
    },
    {
      // Open any url that includes the string "workplace" in Firefox
      match: ["*thefacebook.com/*"],
      browser: "Google Chrome",
    },
    {
      // Open any url that includes the string "workplace" in Firefox
      match: ["*facebook.csod.com/*", "*facebook.zoom.us/*", "meta.zoom.us/*"],
      browser: "Google Chrome",
    },
    {
      // Open any url that includes the string "workplace" in Firefox
      match: [
        "fburl.com/*",
        "*internalfb.com/*",
        "*fb.facebook.com/*",
        "*fb.workplace.com/*",
        "*docs.google.com*",
        "*figma*",
        "*fburl.com/*",
        "*internalfb.com/*",
        "*fb.facebook.com/*",
        "fb.okta.com/*",
        "*fb.workplace.com/*",
        "https://drive.google.com/drive/folders/*",
        "https://docs.google.com/*",
        "*fb.quip.com*",
      ],
      browser: "Google Chrome",
    },
  ],
};
