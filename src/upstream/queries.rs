pub const STOP_DEPARTURES: &str = "\
query StopDepartures($id: String!, $timeRange: Int!, $numberOfDepartures: Int!) {
  stop(id: $id) {
    gtfsId
    code
    name
    stoptimesWithoutPatterns(timeRange: $timeRange, numberOfDepartures: $numberOfDepartures, omitCanceled: false) {
      scheduledDeparture
      realtimeDeparture
      realtime
      realtimeState
      serviceDay
      headsign
      trip {
        gtfsId
        tripHeadsign
        route { shortName mode }
      }
    }
  }
}";
