// ~/.finicky.js

module.exports = {
  defaultBrowser: "Microsoft Edge",
  rewrite: [
    {
      // Redirect all urls to use https
      match: ({ url }) => url.protocol === "http", url: { protocol: "https" }
    }
  ],
  handlers: [
    {
      // Open apple.com and example.org urls in Safari
      match: ["apple.com/*", "example.org/*"],
      browser: "Safari"
    },
    {
      match: ["*gmail.com/*", "*mail.google.com*"],
      browser: "Microsoft Edge"
    },
    {
      // Open any url that includes the string "workplace" in Firefox
        match: ["fb.workplace.com/*"],
      browser: "Google Chrome"
    },
    {
      // Open any url that includes the string "workplace" in Firefox
        match: ["fburl.com/*","*internalfb.com/*","*fb.facebook.com/*","*fb.workplace.com/*","*docs.google.com*","*figma*"],
      browser: "Google Chrome"
    },
    {
      // Open google.com and *.google.com urls in Google Chrome
      match: [
        "google.com/*", // match google.com urls
        "*.google.com/*", // match google.com subdomains
      ],
      browser: "Google Chrome"
    }
  ]
};
