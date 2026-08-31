mod order;
mod reservation;

use qefro_api::{InstalledApp, LogNotificationJob, OperationDef};

pub fn register(app: InstalledApp) -> InstalledApp {
    app.operation(
        OperationDef::new("confirm", "Reservation")
            .label("Confirm")
            .description("Confirm a pending restaurant reservation")
            .permission("reservation.confirm")
            .roles(&["Manager", "Staff"])
            .transition("confirm")
            .event("reservation.confirmed")
            .job("notify_reservation_confirmed"),
        reservation::ConfirmReservation,
    )
    .operation(
        OperationDef::new("seat_customer", "Reservation")
            .label("Seat Customer")
            .description("Seat a confirmed reservation and occupy the table")
            .permission("reservation.seat_customer")
            .roles(&["Manager", "Staff"])
            .transition("seat")
            .event("reservation.seated")
            .tool("seat_customer"),
        reservation::SeatCustomer,
    )
    .operation(
        OperationDef::new("complete", "Reservation")
            .label("Complete")
            .description("Complete a seated reservation and free the table")
            .permission("reservation.complete")
            .roles(&["Manager", "Staff"])
            .transition("complete")
            .event("reservation.completed"),
        reservation::CompleteReservation,
    )
    .operation(
        OperationDef::new("cancel", "Reservation")
            .label("Cancel")
            .description("Cancel a reservation according to current state")
            .permission("reservation.cancel")
            .confirm()
            .style("danger")
            .event("reservation.cancelled"),
        reservation::CancelReservation,
    )
    .operation(
        OperationDef::new("confirm", "Order")
            .label("Confirm")
            .description("Confirm a draft or scheduled order after validating items")
            .permission("order.confirm")
            .roles(&["Manager", "Staff"])
            .transition("confirm")
            .event("order.confirmed")
            .job("notify_order_confirmed"),
        order::ConfirmOrder,
    )
    .operation(
        OperationDef::new("schedule", "Order")
            .label("Schedule Pickup")
            .description("Hold a takeaway order for a booked pickup time")
            .permission("order.schedule")
            .roles(&["Manager", "Staff"])
            .transition("schedule")
            .event("order.scheduled"),
        order::SchedulePickup,
    )
    .operation(
        OperationDef::new("start_preparation", "Order")
            .label("Start Preparation")
            .permission("order.start_preparation")
            .roles(&["Manager", "Staff"])
            .transition("prepare")
            .event("order.preparing")
            .tool("start_preparation"),
        order::StartPreparation,
    )
    .operation(
        OperationDef::new("mark_ready", "Order")
            .label("Mark Ready")
            .permission("order.mark_ready")
            .roles(&["Manager", "Staff"])
            .transition("ready")
            .event("order.ready")
            .tool("mark_ready"),
        order::MarkReady,
    )
    .operation(
        OperationDef::new("complete", "Order")
            .label("Complete Order")
            .description("Complete the order and create a follow-up task")
            .permission("order.complete")
            .roles(&["Manager", "Staff"])
            .transition("complete")
            .event("order.completed")
            .confirmation_message(
                "This will complete the order and create a follow-up task for the guest.",
            ),
        order::CompleteOrder,
    )
    .operation(
        OperationDef::new("cancel", "Order")
            .label("Cancel")
            .permission("order.cancel")
            .confirm()
            .style("danger")
            .event("order.cancelled"),
        order::CancelOrder,
    )
    .job("notify_reservation_confirmed", LogNotificationJob)
    .job("notify_order_confirmed", LogNotificationJob)
}
