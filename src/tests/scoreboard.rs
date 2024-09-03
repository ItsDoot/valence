use valence_scoreboard::*;

use crate::client::VisibleEntityLayers;
use crate::entity::EntityLayerId;
use crate::layer::EntityLayer;
use crate::protocol::packets::play::{
    ScoreboardDisplayS2c, ScoreboardObjectiveUpdateS2c, ScoreboardPlayerUpdateS2c,
};
use crate::testing::ScenarioSingleClient;
use crate::text::IntoText;
use crate::Server;

#[test]
fn show_scoreboard_when_added_to_layer() {
    let ScenarioSingleClient {
        mut app,
        client,
        mut helper,
        ..
    } = ScenarioSingleClient::new();

    // Add a new entity layer for the objective.
    let server = app.world().get_resource::<Server>().unwrap().clone();
    let obj_layer = app.world_mut().spawn(EntityLayer::new(&server)).id();

    app.world_mut()
        .entity_mut(client)
        .get_mut::<VisibleEntityLayers>()
        .unwrap()
        .0
        .insert(obj_layer);

    // Process a tick to get past the "on join" logic.
    app.update();
    helper.clear_received();

    // Spawn the objective.
    app.world_mut().spawn((
        Objective::new("foo"),
        ObjectiveDisplay("Foo".into_text()),
        EntityLayerId(obj_layer),
    ));

    app.update();

    // Check that the objective was sent to the client.
    {
        let recvd = helper.collect_received();

        recvd.assert_count::<ScoreboardObjectiveUpdateS2c>(1);
        recvd.assert_count::<ScoreboardDisplayS2c>(1);
        recvd.assert_order::<(ScoreboardObjectiveUpdateS2c, ScoreboardDisplayS2c)>();
    }
}

#[test]
fn show_scoreboard_when_client_join() {
    let ScenarioSingleClient {
        mut app,
        client,
        mut helper,
        ..
    } = ScenarioSingleClient::new();

    // Add a new entity layer for the objective.
    let server = app.world().get_resource::<Server>().unwrap().clone();
    let obj_layer = app.world_mut().spawn(EntityLayer::new(&server)).id();
    app.world_mut()
        .entity_mut(client)
        .get_mut::<VisibleEntityLayers>()
        .unwrap()
        .0
        .insert(obj_layer);

    // Spawn the objective.
    app.world_mut().spawn((
        Objective::new("foo"),
        ObjectiveDisplay("Foo".into_text()),
        EntityLayerId(obj_layer),
    ));

    // Process a tick to get past the "on join" logic.
    app.update();

    // Check that the objective was sent to the client.
    {
        let recvd = helper.collect_received();

        recvd.assert_count::<ScoreboardObjectiveUpdateS2c>(1);
        recvd.assert_count::<ScoreboardDisplayS2c>(1);
        recvd.assert_order::<(ScoreboardObjectiveUpdateS2c, ScoreboardDisplayS2c)>();
    }
}

#[test]
fn should_update_score() {
    let ScenarioSingleClient {
        mut app,
        client,
        mut helper,
        ..
    } = ScenarioSingleClient::new();

    // Add a new entity layer for the objective.
    let server = app.world_mut().get_resource::<Server>().unwrap().clone();
    let obj_layer = app.world_mut().spawn(EntityLayer::new(&server)).id();
    app.world_mut()
        .entity_mut(client)
        .get_mut::<VisibleEntityLayers>()
        .unwrap()
        .0
        .insert(obj_layer);

    // Spawn the objective.
    let obj = app
        .world_mut()
        .spawn((
            Objective::new("foo"),
            ObjectiveDisplay("Foo".into_text()),
            ObjectiveScores::with_map([("foo".to_owned(), 0)]),
            EntityLayerId(obj_layer),
        ))
        .id();

    // Process a tick to get past the "on join" logic.
    app.update();
    helper.clear_received();

    let mut scores = app.world_mut().get_mut::<ObjectiveScores>(obj).unwrap();
    scores.insert("foo", 3);

    app.update();

    // Check that the objective was sent to the client.
    {
        let recvd = helper.collect_received();

        recvd.assert_count::<ScoreboardPlayerUpdateS2c>(1);
    }
}

#[test]
fn should_only_update_score_diff() {
    let ScenarioSingleClient {
        mut app,
        client,
        mut helper,
        ..
    } = ScenarioSingleClient::new();

    // Add a new entity layer for the objective.
    let server = app.world().get_resource::<Server>().unwrap().clone();
    let obj_layer = app.world_mut().spawn(EntityLayer::new(&server)).id();
    app.world_mut()
        .entity_mut(client)
        .get_mut::<VisibleEntityLayers>()
        .unwrap()
        .0
        .insert(obj_layer);

    // Spawn the objective.
    let obj = app
        .world_mut()
        .spawn((
            Objective::new("foo"),
            ObjectiveDisplay("Foo".into_text()),
            ObjectiveScores::with_map([("foo".to_owned(), 0), ("bar".to_owned(), 0)]),
            EntityLayerId(obj_layer),
        ))
        .id();

    // Process a tick to get past the "on join" logic.
    app.update();
    helper.clear_received();

    let mut scores = app.world_mut().get_mut::<ObjectiveScores>(obj).unwrap();
    scores.insert("foo", 3);

    app.update();

    // Check that the objective was sent to the client.
    {
        let recvd = helper.collect_received();

        recvd.assert_count::<ScoreboardPlayerUpdateS2c>(1);
    }
}
