import { registerView } from "./registry";
import ListView from "../components/views/ListView";
import KanbanView from "../components/views/KanbanView";
import CalendarView from "../components/views/CalendarView";

registerView("list", ListView);
registerView("kanban", KanbanView);
registerView("calendar", CalendarView);
