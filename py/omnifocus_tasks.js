// https://omni-automation.com/omnifocus/task.html-
// GET ALL PROJECTS
//const allProjects =Application("OmniFocus").defaultDocument.flattenedProjects()
const allProjects = [];
const allTasks = Application("OmniFocus").defaultDocument.flattenedTasks();

// EXTRACT PROJECT DATA TO SCRIPT FILTER ITEMS
const sfItems = allProjects.map((project) => {
  const projName = project.name();
  const projID = project.id();
  const taskCount = project.numberOfTasks();
  const remainTaskCount = taskCount - project.numberOfCompletedTasks();

  return {
    title: projName,
    subtitle: `${remainTaskCount} tasks remaining of ${taskCount}`,
    arg: projID,
  };
});

const mapTasks = allTasks.map((task) => {
  return {
    name: task.name(),
    id: task.id(),
    completed: task.completed(),
    flagged: task.flagged(),
    tags: task.tags(),
    in_inbox: task.inInbox(),
    note: task.note(),
  };
});

// OUTPUT JSON
//JSON.stringify({allProjects})
//JSON.stringify(sfItems)
//JSON.stringify(allTasks)
JSON.stringify(mapTasks);
