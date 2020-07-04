#!/usr/bin/env osascript -l JavaScript

/**
 * A JXA script and an Alfred Workflow for controlling Google Chrome (Javascript for Automation).
 * Also see my "How I Navigate Hundreds of Tabs on Chrome with JXA and Alfred" article at [1]
 * if you're interested in learning how I created the workflow.
 * [1] https://medium.com/@bit2pixel/how-i-navigate-hundreds-of-tabs-on-chrome-with-jxa-and-alfred-9bbf971af02b
 */

ObjC.import('stdlib')
ObjC.import('Foundation')

const chrome = Application('Google Chrome')
chrome.includeStandardAdditions = true

// Mode flags
const MODE_CLI = 0    // Ask questions in command line
const MODE_UI = 1     // Ask questions with Chrome dialogs
const MODE_YES = 2    // Answer all questions with `yes`
let MODE = MODE_CLI   // Default mode is command line

// Print the usage message
function usage() {
    println('\n--------------')
    println('Chrome Control')
    println('--------------\n')
    println('list                        List all open tabs in all Chrome windows          usage: ./chrome.js list')
    println('dedup                       Close duplicate tabs                              usage: ./chrome.js dedup')
    println('close <winIdx,tabIdx>       Close a specific tab in a specific window         usage: ./chrome.js close 0,13')
    println('close --title <string(s)>   Close all tabs with titles containing strings     usage: ./chrome.js close --title Inbox "iphone - apple"')
    println('close --url <string(s)>     Close all tabs with URLs containing strings       usage: ./chrome.js close --url mail.google apple')
    println('focus <winIdx,tabIdx>       Focus on a specific tab in a specific window      usage: ./chrome.js focus 0,13')
    println('--ui                        If set, use Chrome to show messages               usage  ./chrome.js close --title inbox --ui')
    println('--yes                       If set, all questions will be anwered with "y"    usage  ./chrome.js close --title inbox --yes')
    $.exit(1)
}

// Run Chrome Control and catch all exceptions
function run(argv) {
    try {
        chromeControl(argv)
    } catch (e) {
        println(e)
    }
}

function getAllUrlParams(url) {

    // get query string from url (optional) or window
    var queryString = url ? url.split('?')[1] : window.location.search.slice(1);

    // we'll store the parameters here
    var obj = {};

    // if query string exists
    if (queryString) {

        // stuff after # is not part of query string, so get rid of it
        queryString = queryString.split('#')[0];

        // split our query string into its component parts
        var arr = queryString.split('&');

        for (var i = 0; i < arr.length; i++) {
            // separate the keys and the values
            var a = arr[i].split('=');

            // set parameter name and value (use 'true' if empty)
            var paramName = a[0];
            var paramValue = typeof (a[1]) === 'undefined' ? true : a[1];

            // (optional) keep case consistent
            paramName = paramName.toLowerCase();
            if (typeof paramValue === 'string') paramValue = paramValue.toLowerCase();

            // if the paramName ends with square brackets, e.g. colors[] or colors[2]
            if (paramName.match(/\[(\d+)?\]$/)) {

                // create key if it doesn't exist
                var key = paramName.replace(/\[(\d+)?\]/, '');
                if (!obj[key]) obj[key] = [];

                // if it's an indexed array e.g. colors[2]
                if (paramName.match(/\[\d+\]$/)) {
                    // get the index value and add the entry at the appropriate position
                    var index = /\[(\d+)\]/.exec(paramName)[1];
                    obj[key][index] = paramValue;
                } else {
                    // otherwise add the value to the end of the array
                    obj[key].push(paramValue);
                }
            } else {
                // we're dealing with a string
                if (!obj[paramName]) {
                    // if it doesn't exist, create property
                    obj[paramName] = paramValue;
                } else if (obj[paramName] && typeof obj[paramName] === 'string'){
                    // if property does exist and it's a string, convert it to an array
                    obj[paramName] = [obj[paramName]];
                    obj[paramName].push(paramValue);
                } else {
                    // otherwise add the property
                    obj[paramName].push(paramValue);
                }
            }
        }
    }

    return obj
}

function GetWorkPlaceProfile(name)
{
    const internProfile = `https://www.internalfb.com/intern/bunny/?q=wpp%20${encodeURIComponent(name)}`
    return internProfile;
}

function GetInternProfile(name)
{
    const internProfile = `https://www.internalfb.com/intern/bunny/?q=ip%20${encodeURIComponent(name)}`
    return internProfile;
}

function CreateTab(url)
{
    const tab = new chrome.Tab({"url":url})
    return tab;
}

function GetId(url)
{
    const id = new getAllUrlParams(url).id
    return id
}

function openForPeople()
{
    const activeTab =  chrome.windows[0].activeTab
    const url =  activeTab.url()
    const title =  activeTab.title()
    println(url)
    println(title)
    const isIntern = url.startsWith("https://www.internalfb.com/intern/profile/")
    const isWorkplace = url.startsWith("https://fb.workplace.com/profile.php" )
    var name = "NOT FOUND"
    if (isIntern)
    {
        name = title.split(':')[0]
    }
    if (isWorkplace)
    {
        name = title
    }
    if (name == "NOT FOUND")
    {
        println("Not a Profile Page")
        return;
    }
    println (name)
    const searchParam = encodeURIComponent(`${name} facebook`)
    const linkedInLink = `https://www.linkedin.com/search/resul.ts/all/?keywords=${searchParam}`
    println("LinkedIn Search:\n  " + linkedInLink)

    const internProfile = GetInternProfile(name)
    println("Intern Profile:\n  " + internProfile)


    const workplaceProfile = GetWorkPlaceProfile(name)
    println("Workplace Profile:\n  " + workplaceProfile)


}

function AllTabsForPerson(name="Pablo Bravo")
{
    println("hello")

    // https://stackoverflow.com/questions/27391033/how-can-i-open-a-new-chrome-window-with-javascript-automation-in-yosemite
    // https://medium.com/@SteveBarbera/automating-chrome-with-jxa-javascript-application-scripting-6f9bc433216a
    const intern = CreateTab(GetInternProfile(name))
    const wp = CreateTab(GetWorkPlaceProfile(name))
    const w = chrome.Window().make()
    const newTab = w.tabs[0]
    w.tabs.push(wp)
    w.tabs.push(intern)
    newTab.close()
    // debugger
    println (`For ${name}:`)
    delay(2) // Delay in seconds
    println (intern.url())
    const internId = GetId(intern.url())
    const wpId = GetId(wp.url())
    w.tabs.push(CreateTab(`https://fb.workplace.com/work/org/${wpId}/`))
    w.tabs.push(CreateTab(`https://fb.workplace.com/chat/t/${wpId}/`))
    w.tabs.push(CreateTab(`https://www.internalfb.com/intern/org/chart/?id=${internId}`))
    const linkedInSearchParam = encodeURIComponent(`${name} facebook`)
    const linkedInUrl = `https://www.linkedin.com/search/results/all/?keywords=${linkedInSearchParam}`
    w.tabs.push(CreateTab(linkedInUrl))
    w.tabs.push(CreateTab(`https://www.internalfb.com/intern/meeting/?invitees[0]=${internId}`))
    w.tabs[0].focus()
        /*
    println("Tab--")
    for (const t of w.tabs)
    {
    //    t.close()
    }
    */
}

// Chrome Control
function chromeControl(argv) {
    if (argv.length < 1) { usage() }

    // --ui flag will cause the questions to be asked using a
    // Chrome dialog instead of text in command line.
    let uiFlagIdx = argv.indexOf('--ui')
    if (uiFlagIdx > -1) {
        MODE = MODE_UI
        argv.splice(uiFlagIdx, 1)
    }

    // --yes flag will cause no questions to be asked to the user.
    // It'll close all tabs straight away so use it with caution.
    let yesFlagIdx = argv.indexOf('--yes')
    if (yesFlagIdx > -1) {
        MODE = MODE_YES
        argv.splice(yesFlagIdx, 1)
    }

    const cmd = argv[0]
    if (cmd === 'list') {
        list('all')
    } else if (cmd === 'dedup') {
        dedup()
    } else if (cmd === 'p') {
        if (argv.length == 1) { usage() }
        const arg = argv[1]
        AllTabsForPerson(arg)
        $.exit(0)
    } else if (cmd === 'close') {
        if (argv.length == 1) { usage() }
        if (argv.length == 2) {
            const arg = argv[1]
            closeTab(arg)
            $.exit(0)
        }
        const subcmd = argv[1]
        const keywords = argv.slice(2, argv.length)
        closeByKeyword(subcmd, keywords)
    } else if (cmd === 'focus') {
        if (argv.length !== 2) { usage() }
        const arg = argv[1]
        focus(arg)
    } else {
        usage()
    }

    $.exit(0)
}

/**
 * Commands
 */

// List all open tabs
function list() {
    // Iterate all tabs in all windows
    let urlToTitle = {}
    chrome.windows().forEach((window, winIdx) => {
        window.tabs().forEach((tab, tabIdx) => {
            urlToTitle[tab.url()] = {
                'title': tab.title() || 'No Title',
                'url': tab.url(),
                'winIdx': winIdx,
                'tabIdx': tabIdx,

                // Alfred specific properties
                'arg': `${winIdx},${tabIdx}`,
                'subtitle': tab.url(),
            }
        })
    })

    // Create a title to url map
    let titleToUrl = {}
    Object.keys(urlToTitle).forEach(url => {
        titleToUrl[urlToTitle[url].title] = urlToTitle[url]
    })

    // Generate output
    out = { 'items': [] }
    Object.keys(titleToUrl).sort().forEach(title => {
        out.items.push(titleToUrl[title])
    })

    // Print output
    println(JSON.stringify(out))
}

// Close a specific tab
function closeTab(arg) {
    let { winIdx, tabIdx } = parseWinTabIdx(arg)

    let tabToClose = chrome.windows[winIdx].tabs[tabIdx]

    // Ask the user before closing tab
    areYouSure([tabToClose], 'Close this tab?', 'Couldn\'t find any matching tabs')

    tabToClose.close()
}

// Close a tab if strings are found in the title or URL
function closeByKeyword(cmd, keywords) {
    if (cmd === '--title') {
        getProperty = function (tab) { return tab.title() }
    }
    else if (cmd === '--url') {
        getProperty = function (tab) { return tab.url() }
    } else {
        usage()
    }

    let tabsToClose = []

    // Iterate all tabs in all windows and compare the property returned
    // by `getProperty` to the given keywords
    chrome.windows().forEach(window => {
        window.tabs().forEach(tab => {
            keywords.forEach(keyword => {
                if (getProperty(tab).toLowerCase().includes(keyword.toLowerCase())) {
                    tabsToClose.push(tab)
                }
            })
        })
    })

    // Ask the user before closing tabs
    areYouSure(tabsToClose, 'Close these tabs?', 'Couldn\'t find any matching tabs')

    // Close tabs
    tabsToClose.forEach(tab => { tab.close() })
}

// Focus on a specific tab
function focus(arg) {
    let { winIdx, tabIdx } = parseWinTabIdx(arg)
    chrome.windows[winIdx].visible = true
    chrome.windows[winIdx].activeTabIndex = tabIdx + 1 // Focous on tab
    chrome.windows[winIdx].index = 1 // Focus on this specific Chrome window
    chrome.activate()
}

// Close duplicate tabs
function dedup() {
    let urls = {}
    let dups = []

    chrome.windows().forEach(window => {
        window.tabs().forEach(tab => {
            const url = tab.url();
            if (urls[url] === undefined) {
                urls[url] = null
            } else {
                dups.push(tab)
            }
        })
    })

    // Ask the user before closing tabs
    areYouSure(dups, 'Close these duplicates?', 'No duplicates found')

    // Close tabs
    dups.forEach(tab => { tab.close() })
}

/**
 * Helpers
 */

// Show a message box in Chrome
const alert = function (msg) {
    if (MODE === MODE_YES) {
        return
    }
    chrome.activate()
    chrome.displayAlert(msg)
}

// Grab input from the command line and return it
const prompt = function (msg) {
    if (MODE === MODE_YES) {
        return 'y'
    } else if (MODE === MODE_UI) {
        chrome.activate()
        chrome.displayDialog(msg)
        return
    }
    println(`\n${msg} (y/N)`)
    return $.NSString.alloc.initWithDataEncoding(
        $.NSFileHandle.fileHandleWithStandardInput.availableData,
        $.NSUTF8StringEncoding
    ).js.trim()
}

// JXA always prints to stderr, so we need this custom print function
const print = function (msg) {
    $.NSFileHandle.fileHandleWithStandardOutput.writeData(
        $.NSString.alloc.initWithString(String(msg))
            .dataUsingEncoding($.NSUTF8StringEncoding)
    )
}

// Print with a new line at the end
const println = function (msg) {
    print(msg + '\n')
}

// Ask the user before closing tabs
function areYouSure(tabsToClose, promptMsg, emptyMsg) {
    // Give user feedback if no matching tabs were found
    if (tabsToClose.length === 0) {
        if (MODE == MODE_CLI) {
            println(emptyMsg)
        } else {
            alert(emptyMsg)
        }

        $.exit(0)
    }

    // Grab the titles to show to the user
    let titles = []
    tabsToClose.forEach(tab => {
        titles.push(tab.title())
    })

    // Focus on Chrome and ask user if they really want to close these tabs
    if (MODE == MODE_CLI) {
        println(`\n${titles.join('\n\n')}`)
        if (prompt(promptMsg) !== 'y') {
            println('Canceled')
            $.exit(0)
        }
    } else {
        prompt(`${promptMsg}\n\n${titles.join('\n\n')}`)
    }

}

// Get winIdx and tabIdx from arg
function parseWinTabIdx(arg) {
    const s = arg.split(',')
    if (s.length !== 2) {
        println('\nInvalid window and tab index. Example: 0,13\n')
        usage()
    }

    let winIdx = parseInt(s[0])
    let tabIdx = parseInt(s[1])

    if (isNaN(winIdx) || isNaN(tabIdx)) {
        throw ("Error: winIdx and tabIdx must be integers")
    }

    return { winIdx, tabIdx }
}
