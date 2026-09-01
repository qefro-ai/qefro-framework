import { registerView } from "./registry";
import ListView from "../components/views/ListView";
import CardView from "../components/views/CardView";
import KanbanView from "../components/views/KanbanView";
import CalendarView from "../components/views/CalendarView";
import ChartView from "../components/views/ChartView";

registerView("list", ListView);
registerView("card", CardView);
registerView("kanban", KanbanView);
registerView("calendar", CalendarView);
registerView("chart", ChartView);
